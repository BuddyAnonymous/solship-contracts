import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Solship } from "../target/types/solship";
import { PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";

describe("solship", () => {
	// Configure the client to use the local cluster.
	anchor.setProvider(anchor.AnchorProvider.env());

	const program = anchor.workspace.Solship as Program<Solship>;

	it("Initialize queue", async () => {
		airdropLamports("TN9afBn533hvXpQ1s5uexBUksR7yMUMjcfgLLc1QKrz", 1000 * LAMPORTS_PER_SOL);
		airdropLamports("4zvwRjXUKGfvwnParsHAS3HuSVzV5cA4McphgmoCtajS", 1000 * LAMPORTS_PER_SOL);

		const tx = await program.methods.initializeQueue().rpc();

		console.log("Transaction signature: ", tx);
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