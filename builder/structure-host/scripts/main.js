import { BlockPermutation, GameMode, Player, system, world } from "@minecraft/server";

const uploads = new Map();
const results = new Map();
const MAX_BYTES = 384 * 1024;
// Sections running at once. More wait in a queue; none are dropped.
const MAX_RUNNING = 8;
const MAX_QUEUED = 20000;
// Where saved structures go. Without a namespace a structure block's Load box looks here, so
// players type only the name.
const SAVED_NAMESPACE = "mystructure:";
const MAX_SAVED_SIDE = 64;
const MAX_SAVED_HEIGHT = 384;
let serial = 0;

system.runInterval(() => {
    for (const [player, upload] of uploads) {
        if (system.currentTick - upload.tick > 200) uploads.delete(player);
    }
    for (const [player, result] of results) {
        if (system.currentTick - result.tick > 1200 && result.pending === 0 && result.queue.length === 0) {
            if (result.saved) {
                try { world.structureManager.delete(result.saved.structure); } catch (error) { /* gone */ }
            }
            results.delete(player);
        }
    }
}, 100);

function compilePalette(palette, volume) {
    if (!Array.isArray(palette) || palette.length > volume) throw new Error("invalid template palette");
    const compiled = [];
    for (const entry of palette) {
        // A block this world can't make is skipped on its own; the rest of the section stays.
        let item = null;
        try {
            const [type, states] = entry;
            if (/^minecraft:[a-z0-9_]+$/.test(type) && states && typeof states === "object" &&
                !Array.isArray(states) && Object.entries(states).every(([name, value]) =>
                    /^[a-z0-9_:]+$/.test(name) && ["number", "boolean", "string"].includes(typeof value))) {
                item = { type, states, permutation: BlockPermutation.resolve(type, states) };
            }
        } catch (error) {
            item = null;
        }
        compiled.push(item);
    }
    return compiled;
}

function* exportSection(player, result, payload) {
    const [origin, size, , palette, runs] = JSON.parse(payload);
    const saved = result.saved;
    const isTriplet = (value, test) => Array.isArray(value) && value.length === 3 &&
        value.every(number => Number.isSafeInteger(number) && test(number));
    if (!isTriplet(origin, value => value >= 0) || !isTriplet(size, value => value >= 1 && value <= 16) ||
        !Array.isArray(runs) || origin[0] + size[0] > saved.size[0] || origin[1] + size[1] > saved.size[1] ||
        origin[2] + size[2] > saved.size[2]) {
        throw new Error("invalid section");
    }
    const [sx, sy, sz] = size;
    const volume = sx * sy * sz;
    if (runs.length > volume) throw new Error("oversized section");
    const compiled = compilePalette(palette, volume);
    yield;
    let end = 0;
    let changed = 0;
    for (const [start, length, index] of runs) {
        if (![start, length, index].every(Number.isSafeInteger) || start < end ||
            length <= 0 || start + length > volume || index < 0 || index >= compiled.length) {
            throw new Error("invalid template run");
        }
        end = start + length;
        const item = compiled[index];
        if (!item) {
            result.unsupported += length;
            continue;
        }
        for (let cell = start; cell < end; cell++) {
            if (results.get(player.id) !== result || !saved.structure.isValid) return 0;
            if ((cell & 15) === 0) yield;
            saved.structure.setBlockPermutation({
                x: origin[0] + (cell % sx),
                y: origin[1] + Math.floor(cell / (sx * sz)),
                z: origin[2] + (Math.floor(cell / sx) % sz),
            }, item.permutation);
            changed++;
        }
    }
    return changed;
}

function finishSaved(player, result) {
    const saved = result.saved;
    const manager = world.structureManager;
    const id = SAVED_NAMESPACE + saved.name;
    try {
        // A structure saved earlier under this name is replaced.
        manager.delete(id);
        saved.structure.saveAs(id);
        player.sendMessage("Cinnamatica | Structure Export: saved " + saved.name + " (" +
            saved.size.join(" x ") + ", " + result.changed + " blocks" +
            (result.unsupported > 0 ? ", " + result.unsupported + " unsupported blocks left out" : "") +
            (result.skipped > 0 ? ", " + result.skipped + " sections skipped" : "") +
            "). In a structure block choose Load and type " + saved.name);
    } catch (error) {
        player.sendMessage("Cinnamatica | Structure Export: could not save " + saved.name + ": " + error.message);
    } finally {
        try { manager.delete(saved.structure); } catch (error) { /* already gone */ }
    }
}

function* placeSection(player, result, payload) {
    const [origin, size, replace, palette, runs] = JSON.parse(payload);
    const isTriplet = (value, test) => Array.isArray(value) && value.length === 3 &&
        value.every(number => Number.isSafeInteger(number) && test(number));
    if (!isTriplet(origin, value => Math.abs(value) <= 30000000) ||
        !isTriplet(size, value => value >= 1 && value <= 16) ||
        typeof replace !== "boolean" || !Array.isArray(runs)) {
        throw new Error("invalid section");
    }
    const [sx, sy, sz] = size;
    const volume = sx * sy * sz;
    if (runs.length > volume) throw new Error("oversized section");
    const compiled = compilePalette(palette, volume);
    yield;

    const dimension = player.dimension;
    const manager = world.structureManager;
    const id = "cinnamatica:paste_" + (++serial);
    const structure = manager.createEmpty(id, { x: sx, y: sy, z: sz });
    try {
        let end = 0;
        let changed = 0;
        for (const [start, length, index] of runs) {
            if (![start, length, index].every(Number.isSafeInteger) || start < end ||
                length <= 0 || start + length > volume || index < 0 || index >= compiled.length) {
                throw new Error("invalid template run");
            }
            end = start + length;
            const item = compiled[index];
            if (!item) {
                result.unsupported += length;
                continue;
            }
            const { type, states, permutation } = item;
            for (let cell = start; cell < end; cell++) {
                if (results.get(player.id) !== result || player.dimension.id !== dimension.id) return 0;
                if ((cell & 15) === 0) yield;
                const x = cell % sx;
                const y = Math.floor(cell / (sx * sz));
                const z = Math.floor(cell / sx) % sz;
                const current = dimension.getBlock({ x: origin[0] + x, y: origin[1] + y, z: origin[2] + z });
                if (!current) throw new Error("destination isn't loaded");
                if (!current.isAir && (!replace || current.permutation.matches(type, states))) continue;
                structure.setBlockPermutation({ x, y, z }, permutation);
                changed++;
            }
        }
        if (changed > 0 && results.get(player.id) === result && player.dimension.id === dimension.id) {
            manager.place(structure, dimension, { x: origin[0], y: origin[1], z: origin[2] }, { includeEntities: false });
        }
        return changed;
    } finally {
        manager.delete(structure);
    }
}

function report(player, result) {
    if (result.pending !== 0 || result.queue.length !== 0 || !result.finished || results.get(player.id) !== result) return;
    results.delete(player.id);
    if (result.saved) {
        finishSaved(player, result);
        return;
    }
    player.sendMessage("Cinnamatica | Structure Paste: " + result.changed +
        " blocks placed, " + result.skipped + " sections skipped" +
        (result.unsupported > 0 ? ", " + result.unsupported + " unsupported blocks left out" : ""));
}

function newResult(saved) {
    return {
        changed: 0, skipped: 0, unsupported: 0, pending: 0, finished: false,
        queue: [], saved: saved ?? null, tick: system.currentTick,
    };
}

function* processSection(player, result, payload) {
    try {
        result.changed += yield* (result.saved ? exportSection : placeSection)(player, result, payload);
    } catch (error) {
        result.skipped++;
        if (results.get(player.id) === result) {
            player.sendMessage("Cinnamatica | Structure section skipped: " + error.message);
        }
    } finally {
        result.pending--;
        pump(player, result);
        report(player, result);
    }
}

// Starts queued sections while fewer than MAX_RUNNING are running.
function pump(player, result) {
    if (results.get(player.id) !== result) return;
    while (result.pending < MAX_RUNNING && result.queue.length > 0) {
        result.pending++;
        system.runJob(processSection(player, result, result.queue.shift()));
    }
}

function discard(player) {
    const result = results.get(player.id);
    results.delete(player.id);
    if (result?.saved) {
        try { world.structureManager.delete(result.saved.structure); } catch (error) { /* already gone */ }
    }
}

system.afterEvents.scriptEventReceive.subscribe(event => {
    const player = event.sourceEntity;
    if (event.id !== "cinnamatica:structure" || !(player instanceof Player) ||
        player.getGameMode() !== GameMode.Creative) return;
    if (event.message === "begin") {
        uploads.delete(player.id);
        discard(player);
        results.set(player.id, newResult());
        return;
    }
    const save = /^save ([a-z0-9_]{1,32}) (\d{1,3}) (\d{1,3}) (\d{1,3})$/.exec(event.message);
    if (save) {
        uploads.delete(player.id);
        discard(player);
        const size = [Number(save[2]), Number(save[3]), Number(save[4])];
        if (size[0] < 1 || size[1] < 1 || size[2] < 1 || size[0] > MAX_SAVED_SIDE ||
            size[2] > MAX_SAVED_SIDE || size[1] > MAX_SAVED_HEIGHT) {
            player.sendMessage("Cinnamatica | Structure Export: " + save[1] + " is too big for a structure block");
            return;
        }
        try {
            const structure = world.structureManager.createEmpty("cinnamatica:export_" + (++serial), {
                x: size[0], y: size[1], z: size[2],
            });
            results.set(player.id, newResult({ name: save[1], size, structure }));
        } catch (error) {
            player.sendMessage("Cinnamatica | Structure Export: could not start " + save[1] + ": " + error.message);
        }
        return;
    }
    if (event.message === "cancel") {
        uploads.delete(player.id);
        discard(player);
        return;
    }
    if (event.message === "done" || event.message === "saved") {
        uploads.delete(player.id);
        const result = results.get(player.id);
        if (result && Boolean(result.saved) === (event.message === "saved")) {
            result.finished = true;
            report(player, result);
        }
        return;
    }
    const part = /^(\d{1,3}) ([01]) (.+)$/.exec(event.message);
    if (!part) return;
    const sequence = Number(part[1]);
    if (sequence === 0) {
        uploads.set(player.id, { next: 0, dimension: player.dimension.id, tick: system.currentTick, fragments: [], bytes: 0 });
    }
    const upload = uploads.get(player.id);
    if (!upload || upload.next !== sequence || upload.dimension !== player.dimension.id ||
        upload.bytes + part[3].length > MAX_BYTES) {
        uploads.delete(player.id);
        player.sendMessage("Cinnamatica | Structure upload interrupted");
        return;
    }
    upload.next++;
    upload.tick = system.currentTick;
    upload.bytes += part[3].length;
    upload.fragments.push(part[3]);
    if (part[2] === "1") {
        uploads.delete(player.id);
        const result = results.get(player.id);
        if (!result || result.finished) return;
        if (result.queue.length >= MAX_QUEUED) {
            result.skipped++;
            return;
        }
        result.tick = system.currentTick;
        result.queue.push(upload.fragments.join(""));
        pump(player, result);
    }
}, { namespaces: ["cinnamatica"] });
