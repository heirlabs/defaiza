import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DefaiSwap } from "../../target/types/defai_swap";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";
import { expectAnchorError, sleep } from "../helpers/assert_anchor_error";

describe("blocker governance init", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.DefaiSwap as Program<DefaiSwap>;

  it("rejects non-governor swap initialize", async () => {
    const attacker = Keypair.generate();
    await provider.connection.requestAirdrop(attacker.publicKey, 2e9);
    await sleep(1000);

    const [config] = PublicKey.findProgramAddressSync([Buffer.from("config")], program.programId);
    const [escrow] = PublicKey.findProgramAddressSync([Buffer.from("escrow")], program.programId);
    const [taxState] = PublicKey.findProgramAddressSync([Buffer.from("tax_state")], program.programId);

    try {
      await program.methods
        .initialize([1, 2, 3, 4, 5].map((n) => new anchor.BN(n)))
        .accounts({
          admin: attacker.publicKey,
          oldMint: Keypair.generate().publicKey,
          newMint: Keypair.generate().publicKey,
          collection: Keypair.generate().publicKey,
          treasury: Keypair.generate().publicKey,
          config,
          escrow,
          taxState,
          systemProgram: SystemProgram.programId,
        })
        .signers([attacker])
        .rpc();
      assert.fail("expected UnauthorizedGovernor");
    } catch (err: unknown) {
      expectAnchorError(err, "UnauthorizedGovernor");
    }
  });

  it("rejects initialize with wrong price vector length", async () => {
    const attacker = Keypair.generate();
    await provider.connection.requestAirdrop(attacker.publicKey, 2e9);
    await sleep(1000);

    const [config] = PublicKey.findProgramAddressSync([Buffer.from("config")], program.programId);
    const [escrow] = PublicKey.findProgramAddressSync([Buffer.from("escrow")], program.programId);
    const [taxState] = PublicKey.findProgramAddressSync([Buffer.from("tax_state")], program.programId);

    try {
      await program.methods
        .initialize([1, 2, 3].map((n) => new anchor.BN(n)))
        .accounts({
          admin: attacker.publicKey,
          oldMint: Keypair.generate().publicKey,
          newMint: Keypair.generate().publicKey,
          collection: Keypair.generate().publicKey,
          treasury: Keypair.generate().publicKey,
          config,
          escrow,
          taxState,
          systemProgram: SystemProgram.programId,
        })
        .signers([attacker])
        .rpc();
      assert.fail("expected InvalidInput");
    } catch (err: unknown) {
      expectAnchorError(err, "InvalidInput");
    }
  });
});
