// Simple seeded RNG (linear congruential generator)
export class SeededRNG {
    private seed: number;
    constructor(seed: number) {
        this.seed = seed;
    }

    // Generates a random number between 0 (inclusive) and 1 (exclusive)
    random() {
        // Constants for a linear congruential generator
        const a = 1664525;
        const c = 1013904223;
        const m = 2 ** 32;
        this.seed = (a * this.seed + c) % m;
        return this.seed / m;
    }
}
