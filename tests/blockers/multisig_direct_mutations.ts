import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DefaiEstate } from "../../target/types/defai_estate";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { assert } from "chai";
import { expectAnchorError, sleep } from "../helpers/assert_anchor_error";

describe("blocker multisig direct mutations", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.DefaiEstate as Program<DefaiEstate>;

  async function setupEstateWithMultisig() {
    const owner = Keypair.generate();
    const signer1 = Keypair.generate();
    const signer2 = Keypair.generate();
    await provider.connection.requestAirdrop(owner.publicKey, 5 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(signer1.publicKey, LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(signer2.publicKey, LAMPORTS_PER_SOL);
    await sleep(1500);

    const [globalCounter] = PublicKey.findProgramAddressSync([Buffer.from("counter")], program.programId);
    try {
      await program.methods
        .initializeGlobalCounter()
        .accounts({ admin: provider.wallet.publicKey, globalCounter, systemProgram: SystemProgram.programId })
        .rpc();
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

    return { owner, estate, multisig, signer1, signer2 };
  }

  it("blocks direct enable_trading when multisig is attached", async () => {
    const { owner, estate } = await setupEstateWithMultisig();
    try {
      await program.methods
        .enableTrading(
          Keypair.generate().publicKey,
          70,
          { balanced: {} },
          null,
          new anchor.BN(24)
        )
        .accounts({ owner: owner.publicKey, estate })
        .signers([owner])
        .rpc();
      assert.fail("expected MultisigRequiredForMutation");
    } catch (err: unknown) {
      expectAnchorError(err, "MultisigRequiredForMutation");
    }
  });

  it("blocks direct create_rwa when multisig is attached", async () => {
    const { owner, estate } = await setupEstateWithMultisig();
    const rwaMint = Keypair.generate();
    const [rwa] = PublicKey.findProgramAddressSync(
      [Buffer.from("rwa"), estate.toBuffer(), Buffer.from([0, 0, 0, 0])],
      program.programId
    );

    try {
      await program.methods
        .createRwa("realEstate", "Home", "Primary residence", "500000", "https://example.com/meta")
        .accounts({
          owner: owner.publicKey,
          estate,
          rwa,
          rwaMint: rwaMint.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();
      assert.fail("expected MultisigRequiredForMutation");
    } catch (err: unknown) {
      expectAnchorError(err, "MultisigRequiredForMutation");
    }
  });

  it("rejects beneficiary shares that do not sum to 100", async () => {
    const owner = Keypair.generate();
    await provider.connection.requestAirdrop(owner.publicKey, 5 * LAMPORTS_PER_SOL);
    await sleep(1500);

    const [globalCounter] = PublicKey.findProgramAddressSync([Buffer.from("counter")], program.programId);
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

    try {
      await program.methods
        .updateBeneficiaries([
          { address: owner.publicKey, sharePercentage: 60, claimed: false, emailHash: Array(32).fill(1) },
          { address: owner.publicKey, sharePercentage: 30, claimed: false, emailHash: Array(32).fill(2) },
        ])
        .accounts({ owner: owner.publicKey, estate })
        .signers([owner])
        .rpc();
      assert.fail("expected InvalidBeneficiaryShares");
    } catch (err: unknown) {
      expectAnchorError(err, "InvalidBeneficiaryShares");
    }
  });
});
