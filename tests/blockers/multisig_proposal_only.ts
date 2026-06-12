import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DefaiEstate } from "../../target/types/defai_estate";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { assert } from "chai";
import { expectAnchorError, sleep } from "../helpers/assert_anchor_error";

describe("blocker multisig proposal-only", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.DefaiEstate as Program<DefaiEstate>;

  it("blocks direct beneficiary update when multisig is attached", async () => {
    const owner = Keypair.generate();
    const signer1 = Keypair.generate();
    const signer2 = Keypair.generate();
    await provider.connection.requestAirdrop(owner.publicKey, 5 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(signer1.publicKey, LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(signer2.publicKey, LAMPORTS_PER_SOL);
    await sleep(1500);

    const [globalCounter] = PublicKey.findProgramAddressSync([Buffer.from("counter")], program.programId);
    try {
      await program.methods.initializeGlobalCounter().accounts({ admin: provider.wallet.publicKey, globalCounter, systemProgram: SystemProgram.programId }).rpc();
    } catch {
      /* already initialized */
    }

    const estateMint = Keypair.generate();
    const [estate] = PublicKey.findProgramAddressSync(
      [Buffer.from("estate"), owner.publicKey.toBuffer(), estateMint.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .createEstate(new anchor.BN(86400), new anchor.BN(86400), Array(32).fill(0))
      .accounts({
        owner: owner.publicKey,
        estate,
        estateMint: estateMint.publicKey,
        globalCounter,
        systemProgram: SystemProgram.programId,
      })
      .signers([owner])
      .rpc();

    const [multisig] = PublicKey.findProgramAddressSync(
      [Buffer.from("multisig"), owner.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .initializeMultisig([signer1.publicKey, signer2.publicKey], 2)
      .accounts({ admin: owner.publicKey, multisig, systemProgram: SystemProgram.programId })
      .signers([owner])
      .rpc();

    await program.methods
      .attachMultisig()
      .accounts({ owner: owner.publicKey, estate, multisig })
      .signers([owner])
      .rpc();

    try {
      await program.methods
        .updateBeneficiaries([{ address: owner.publicKey, sharePercentage: 100, claimed: false, emailHash: Array(32).fill(1) }])
        .accounts({ owner: owner.publicKey, estate })
        .signers([owner])
        .rpc();
      assert.fail("expected MultisigRequiredForMutation");
    } catch (err: unknown) {
      expectAnchorError(err, "MultisigRequiredForMutation");
    }
  });
});
