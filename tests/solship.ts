import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Solship } from "../target/types/solship";
import { PublicKey, LAMPORTS_PER_SOL, Keypair, ComputeBudgetInstruction, ComputeBudgetProgram } from "@solana/web3.js";
import { constructMerkleTree } from "./merkleTree/merkleTree";
import { getFixedBoard1, getFixedBoard2, hexStringToByteArray, printBoard } from "./merkleTree/helpers";

const INITIAL_BALANCE = 1000; // 1000 SOL

describe("solship", () => {
	// Configure the client to use the local cluster.
	anchor.setProvider(anchor.AnchorProvider.env());

	const program = anchor.workspace.Solship as Program<Solship>;

	it("Initialize queue", async () => {
		await airdropLamports("TN9afBn533hvXpQ1s5uexBUksR7yMUMjcfgLLc1QKrz", INITIAL_BALANCE * LAMPORTS_PER_SOL);
		await airdropLamports("4zvwRjXUKGfvwnParsHAS3HuSVzV5cA4McphgmoCtajS", INITIAL_BALANCE * LAMPORTS_PER_SOL);

		const tx = await program.methods.initializeQueue().rpc();

		console.log("Transaction signature: ", tx);
	});

	it("Test claim win", async () => {
		const player1 = Keypair.generate();
		console.log("Player 1:", player1.publicKey.toBase58());
		const player2 = Keypair.generate();
		console.log("Player 2:", player2.publicKey.toBase58());
		await airdropLamports(player1.publicKey.toBase58(), INITIAL_BALANCE * LAMPORTS_PER_SOL);
		await airdropLamports(player2.publicKey.toBase58(), INITIAL_BALANCE * LAMPORTS_PER_SOL);

		const player1Board = getFixedBoard1();
		printBoard(player1Board);
		const player1MerkleRoot = await constructMerkleTree(player1Board);
		console.log("Player 1 Merkle root:", player1MerkleRoot.hash);
		const player2Board = getFixedBoard2();
		printBoard(player2Board);
		const player2MerkleRoot = await constructMerkleTree(player2Board);
		console.log("Player 2 Merkle root:", player2MerkleRoot.hash);

		const tx1 = await program.methods.joinQueue(hexStringToByteArray(player1MerkleRoot.hash))
			.accounts({
				player: player1.publicKey,
			})
			.signers([player1])
			.rpc();

		const tx2 = await program.methods.createGame(player1.publicKey, hexStringToByteArray(player2MerkleRoot.hash))
			.accounts({
				player: player2.publicKey,
			})
			.signers([player2])
			.rpc();

		console.log("GAME: ", await program.account.game.all());

		const gameAddr = (await program.account.game.all())[0].publicKey;
		// Create an array with 28 padding leaves
		const paddingLeaves = Array(28).fill({ shipPlaced: false });
		const player1ClaimWinBoard = player1Board.flat().map(cell => ({ shipPlaced: cell })).concat(paddingLeaves);
		try {
			const tx3 = await program.methods.claimWin(player1ClaimWinBoard)
				.accountsStrict({
					game: gameAddr,
					player: player1.publicKey,
				})
				.preInstructions([
					ComputeBudgetProgram.setComputeUnitLimit({
						units: 1_400_000
					})
				])
				.signers([player1])
				.rpc({
					// skipPreflight: true,
				})
		}
		catch (err) {
			console.log(err);
		}
	});
});

async function airdropLamports(recipient: string, amount: number) {
	const signature = await anchor.getProvider().connection.requestAirdrop(new PublicKey(recipient), amount);

	const latestBlockHash = await anchor.getProvider().connection.getLatestBlockhash();

	await anchor.getProvider().connection.confirmTransaction({
		blockhash: latestBlockHash.blockhash,
		lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
		signature: signature,
	})
}