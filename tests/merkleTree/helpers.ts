export function getFixedBoard1(): boolean[][] {
    // 10x10 board (false represents water, true represents part of a ship)
    const board = Array(10).fill(false).map(() => Array(10).fill(false));

    // Ship 1 (size 5): placed horizontally from (0,0) to (0,4)
    for (let i = 0; i < 5; i++) {
        board[0][i] = true;
    }

    // Ship 2 (size 4): placed vertically from (2,2) to (5,2)
    for (let i = 2; i < 6; i++) {
        board[i][2] = true;
    }

    // Ship 3 (size 3): placed horizontally from (7,5) to (7,7)
    for (let i = 5; i < 8; i++) {
        board[7][i] = true;
    }

    // Ship 4 (size 3): placed vertically from (4,8) to (6,8)
    for (let i = 4; i < 7; i++) {
        board[i][8] = true;
    }

    // Ship 5 (size 2): placed horizontally from (9,5) to (9,8)
    for (let i = 5; i < 7; i++) {
        board[9][i] = true;
    }

    return board;
}

// Another predefined board setup function for testing
export function getFixedBoard2(): boolean[][] {
    // 10x10 board (false represents water, true represents part of a ship)
    const board = Array(10).fill(false).map(() => Array(10).fill(false));

    // Ship 1 (size 5): placed vertically from (0,1) to (4,1)
    for (let i = 0; i < 5; i++) {
        board[i][1] = true;
    }

    // Ship 2 (size 4): placed horizontally from (6,3) to (6,6)
    for (let i = 3; i < 7; i++) {
        board[6][i] = true;
    }

    // Ship 3 (size 3): placed vertically from (3,8) to (5,8)
    for (let i = 3; i < 6; i++) {
        board[i][8] = true;
    }

    // Ship 4 (size 3): placed horizontally from (8,0) to (8,2)
    for (let i = 0; i < 3; i++) {
        board[8][i] = true;
    }

    // Ship 5 (size 2): placed vertically from (1,5) to (4,5)
    for (let i = 1; i < 3; i++) {
        board[i][5] = true;
    }

    return board;
}

export function printBoard(board: boolean[][]): void {
    for (let row of board) {
        console.log(row.map(cell => (cell ? 'X' : 'O')).join(' '));
    }
    console.log("\n");
}


export function hexStringToByteArray(hexString: string): number[] {
    if (hexString.length % 2 !== 0) {
        throw new Error("Invalid hex string");
    }

    const byteArray: number[] = [];

    for (let i = 0; i < hexString.length; i += 2) {
        const byte = parseInt(hexString.substr(i, 2), 16);
        byteArray.push(byte);
    }

    return byteArray;
}