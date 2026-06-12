import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DefaiEstate } from "../../target/types/defai_estate";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { assert } from "chai";
import { expectAnchorError, sleep } from "../helpers/assert_anchor_error";

describe("blocker RWA proposal execution path", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.DefaiEstate as Program<DefaiEstate>;

  it("execute_proposal rejects CreateRWA without dedicated handler accounts", async () => {
    const owner = Keypair.generate();
    const signer1 = Keypair.generate();
    const signer2 = Keypair.generate();
    await provider.connection.requestAirdrop(owner.publicKey, 5 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(signer1.publicKey, LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(signer2.publicKey, LAMPORTS_PER_SOL);
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

    const [multisig] = PublicKey.findProgramAddressSync(
      [Buffer.from("multisig"), owner.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .initializeMultisig([signer1.publicKey, signer2.publicKey, owner.publicKey], 2)
      .accounts({ admin: owner.publicKey, multisig, systemProgram: SystemProgram.programId })
      .signers([owner])
      .rpc();

    await program.methods
      .attachMultisig()
      .accounts({ owner: owner.publicKey, estate, multisig })
      .signers([owner])
      .rpc();

    const [proposal] = PublicKey.findProgramAddressSync(
      [Buffer.from("proposal"), multisig.toBuffer(), Buffer.from([0, 0, 0, 0, 0, 0, 0, 1])],
      program.programId
    );

    await program.methods
      .createProposal(
        {
          createRwa: {
            rwaType: "vehicle",
            name: "Car",
            description: "Tesla",
            value: "40000",
            metadataUri: "https://example.com/car",
          },
        },
        new anchor.BN(1)
      )
      .accounts({
        proposer: owner.publicKey,
        multisig,
        proposal,
        targetEstate: estate,
        systemProgram: SystemProgram.programId,
      })
      .signers([owner])
      .rpc();

    await program.methods
      .approveProposal()
      .accounts({ signer: signer1.publicKey, multisig, proposal })
      .signers([signer1])
      .rpc();

    await program.methods
      .approveProposal()
      .accounts({ signer: signer2.publicKey, multisig, proposal })
      .signers([signer2])
      .rpc();

    try {
      await program.methods
        .executeProposal()
        .accounts({
          executor: owner.publicKey,
          multisig,
          proposal,
          estate,
        })
        .signers([owner])
        .rpc();
      assert.fail("expected UseRwaProposalExecutionInstruction");
    } catch (err: unknown) {
      expectAnchorError(err, "UseRwaProposalExecutionInstruction");
    }
  });
});
