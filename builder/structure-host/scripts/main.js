import { BlockPermutation, GameMode, Player, system, world } from "@minecraft/server";

const uploads = new Map();
const results = new Map();
const MAX_BYTES = 384 * 1024;
let serial = 0;

system.runInterval(() => {
    for (const [player, upload] of uploads) {
        if (system.currentTick - upload.tick > 200) uploads.delete(player);
    }
    for (const [player, result] of results) {
        if (system.currentTick - result.tick > 1200) results.delete(player);
    }
}, 100);

function* placeSection(player, result, payload) {
    const [origin, size, replace, palette, runs] = JSON.parse(payload);
    const isTriplet = (value, test) => Array.isArray(value) && value.length === 3 &&
        value.every(number => Number.isSafeInteger(number) && test(number));
    if (!isTriplet(origin, value => Math.abs(value) <= 30000000) ||
        !isTriplet(size, value => value >= 1 && value <= 16) ||
        typeof replace !== "boolean" || !Array.isArray(palette) || !Array.isArray(runs)) {
        throw new Error("invalid section");
    }
    const [sx, sy, sz] = size;
    const volume = sx * sy * sz;
    if (palette.length > volume || runs.length > volume) throw new Error("oversized section");
    const compiled = [];
    for (const [type, states] of palette) {
        if (results.get(player.id) !== result) return 0;
        if (!/^minecraft:[a-z0-9_]+$/.test(type) || !states || typeof states !== "object" ||
            Array.isArray(states) || Object.entries(states).some(([name, value]) =>
                !/^[a-z0-9_:]+$/.test(name) ||
                !["number", "boolean", "string"].includes(typeof value))) {
            throw new Error("invalid template palette");
        }
        compiled.push({ type, states, permutation: BlockPermutation.resolve(type, states) });
        yield;
    }

    const dimension = player.dimension;
    const manager = world.structureManager;
    const id = "cinnamatica:paste_" + (++serial);
    const structure = manager.createEmpty(id, { x: sx, y: sy, z: sz });
    try {
        let end = 0;
        let changed = 0;
        for (const [start, length, index] of runs) {
            if (![start, length, index].every(Number.isSafeInteger) || start < end ||
                length <= 0 || start + length > volume || !compiled[index]) {
                throw new Error("invalid template run");
            }
            end = start + length;
            const { type, states, permutation } = compiled[index];
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
    if (result.pending !== 0 || !result.finished || results.get(player.id) !== result) return;
    results.delete(player.id);
    player.sendMessage("Cinnamatica | Structure Paste: " + result.changed +
        " blocks placed, " + result.skipped + " sections skipped");
}

function newResult() {
    return { changed: 0, skipped: 0, pending: 0, finished: false, tick: system.currentTick };
}

function* processSection(player, result, payload) {
    try {
        result.changed += yield* placeSection(player, result, payload);
    } catch (error) {
        result.skipped++;
        if (results.get(player.id) === result) {
            player.sendMessage("Cinnamatica | Structure section skipped: " + error.message);
        }
    } finally {
        result.pending--;
        report(player, result);
    }
}

system.afterEvents.scriptEventReceive.subscribe(event => {
    const player = event.sourceEntity;
    if (event.id !== "cinnamatica:structure" || !(player instanceof Player) ||
        player.getGameMode() !== GameMode.Creative) return;
    if (event.message === "begin") {
        uploads.delete(player.id);
        results.set(player.id, newResult());
        return;
    }
    if (event.message === "cancel") {
        uploads.delete(player.id);
        results.delete(player.id);
        return;
    }
    if (event.message === "done") {
        uploads.delete(player.id);
        const result = results.get(player.id);
        if (result) {
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
        if (result.pending >= 8) {
            result.skipped++;
            return;
        }
        result.tick = system.currentTick;
        result.pending++;
        system.runJob(processSection(player, result, upload.fragments.join("")));
    }
}, { namespaces: ["cinnamatica"] });
