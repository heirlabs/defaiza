import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DefaiSwap } from "../../target/types/defai_swap";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expectAnchorError, sleep } from "../helpers/assert_anchor_error";

describe("blocker randomness init governance", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.DefaiSwap as Program<DefaiSwap>;

  it("rejects non-governor randomness initialize", async () => {
    const attacker = Keypair.generate();
    await provider.connection.requestAirdrop(attacker.publicKey, 2e9);
    await sleep(1000);

    const [randomnessState] = PublicKey.findProgramAddressSync(
      [Buffer.from("randomness_state")],
      program.programId
    );

    try {
      await program.methods
        .initializeRandomness()
        .accounts({
          authority: attacker.publicKey,
          randomnessState,
          systemProgram: SystemProgram.programId,
        })
        .signers([attacker])
        .rpc();
      assert.fail("expected UnauthorizedGovernor");
    } catch (err: unknown) {
      expectAnchorError(err, "UnauthorizedGovernor");
    }
  });
});
