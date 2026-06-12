import { createElementSDK } from '@defai/element-sdk';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';

export function registerDeployCommand(program) {
  program
    .command('deploy')
    .description('Deploy a compiled contract to a blockchain network')
    .requiredOption('-b, --bytecode <path>', 'Path to compiled bytecode file')
    .requiredOption('-n, --network <network>', 'Target network (ethereum, polygon, base, avalanche, arbitrum, optimism, solana)')
    .option('-a, --abi <path>', 'Path to ABI JSON file')
    .option('--args <args>', 'Constructor arguments as comma-separated values')
    .option('--private-key <key>', 'Deployer private key (prefer env DEPLOYER_PRIVATE_KEY)')
    .option('--rpc-url <url>', 'RPC URL override')
    .option('--dry-run', 'Simulate deployment without sending transaction')
    .action(async (options) => {
      const bytecodePath = resolve(options.bytecode);
      if (!existsSync(bytecodePath)) {
        console.error(`Bytecode file not found: ${bytecodePath}`);
        process.exit(1);
      }

      const bytecode = readFileSync(bytecodePath, 'utf-8').trim();

      let abi = [];
      if (options.abi) {
        const abiPath = resolve(options.abi);
        if (!existsSync(abiPath)) {
          console.error(`ABI file not found: ${abiPath}`);
          process.exit(1);
        }
        abi = JSON.parse(readFileSync(abiPath, 'utf-8'));
      }

      const constructorArgs = options.args
        ? options.args.split(',').map(arg => {
            const trimmed = arg.trim();
            if (trimmed.match(/^\d+$/)) return BigInt(trimmed);
            if (trimmed === 'true') return true;
            if (trimmed === 'false') return false;
            return trimmed;
          })
        : [];

      const privateKey = options.privateKey || process.env.DEPLOYER_PRIVATE_KEY;
      if (!privateKey) {
        console.error('Deployer private key required. Set DEPLOYER_PRIVATE_KEY env var or use --private-key');
        process.exit(1);
      }

      if (options.dryRun) {
        console.log('DRY RUN - No transaction will be sent');
        console.log(`  Network: ${options.network}`);
        console.log(`  Bytecode size: ${bytecode.length / 2} bytes`);
        console.log(`  Constructor args: ${constructorArgs.length}`);
        console.log(`  ABI functions: ${abi.filter(e => e.type === 'function').length}`);
        return;
      }

      const sdk = createElementSDK();

      console.log(`Deploying to ${options.network}...`);

      const { ethers } = await import('ethers');

      const rpcUrl = options.rpcUrl || process.env[`${options.network.toUpperCase()}_RPC_URL`];
      if (!rpcUrl) {
        console.error(`No RPC URL configured for ${options.network}. Set ${options.network.toUpperCase()}_RPC_URL env var or use --rpc-url`);
        process.exit(1);
      }

      const provider = new ethers.JsonRpcProvider(rpcUrl);
      const signer = new ethers.Wallet(privateKey, provider);

      const result = await sdk.deployContract(bytecode, {
        network: options.network,
        signer,
        bytecode,
        abi,
        constructorArgs,
      });

      console.log('\nDeployment successful!');
      console.log(`  Address: ${result.address}`);
      console.log(`  TX Hash: ${result.txHash}`);
      console.log(`  Chain ID: ${result.chainId}`);
      console.log(`  Block: ${result.blockNumber}`);
      console.log(`  Explorer: ${result.explorerUrl}`);
    });
}
