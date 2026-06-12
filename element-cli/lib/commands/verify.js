import { createElementSDK, EVM_CHAIN_CONFIG } from '@defai/element-sdk';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';

export function registerVerifyCommand(program) {
  program
    .command('verify')
    .description('Verify a deployed contract on block explorers')
    .requiredOption('-a, --address <address>', 'Deployed contract address')
    .requiredOption('-n, --network <network>', 'Network the contract is deployed on')
    .option('-s, --source <path>', 'Path to Solidity source file')
    .option('--contract-name <name>', 'Contract name', 'InheritanceContract')
    .option('--compiler <version>', 'Solidity compiler version', 'v0.8.24+commit.e11b9ed9')
    .option('--optimization', 'Enable optimization flag', false)
    .option('--runs <number>', 'Optimization runs', '200')
    .option('--constructor-args <hex>', 'ABI-encoded constructor arguments (hex)')
    .option('--etherscan-key <key>', 'Etherscan API key (prefer env)')
    .option('--check-only', 'Only check verification status, do not submit')
    .action(async (options) => {
      const chainConfig = EVM_CHAIN_CONFIG[options.network];
      if (!chainConfig) {
        console.error(`Unsupported network: ${options.network}. Supported: ${Object.keys(EVM_CHAIN_CONFIG).join(', ')}`);
        process.exit(1);
      }

      const sdk = createElementSDK();

      if (options.checkOnly) {
        console.log(`Checking verification status for ${options.address} on ${options.network}...`);
        const status = await sdk.checkVerificationStatus(options.address, chainConfig.chainId);
        console.log(`  Verified: ${status.verified}`);
        if (status.sourcify) {
          console.log(`  Sourcify match: ${status.sourcify.matchType || 'none'}`);
        }
        if (status.url) {
          console.log(`  Explorer: ${status.url}`);
        }
        return;
      }

      if (!options.source) {
        console.error('Source file required for verification. Use -s or --source');
        process.exit(1);
      }

      const sourcePath = resolve(options.source);
      if (!existsSync(sourcePath)) {
        console.error(`Source file not found: ${sourcePath}`);
        process.exit(1);
      }

      const sourceCode = readFileSync(sourcePath, 'utf-8');

      const etherscanApiKey = options.etherscanKey
        || process.env[`${options.network.toUpperCase()}_ETHERSCAN_API_KEY`]
        || process.env.ETHERSCAN_API_KEY;

      console.log(`Verifying ${options.address} on ${options.network}...`);
      console.log(`  Contract: ${options.contractName}`);
      console.log(`  Compiler: ${options.compiler}`);

      const result = await sdk.verifyContract(options.address, chainConfig.chainId, {
        sourceCode,
        contractName: options.contractName,
        compilerVersion: options.compiler,
        optimizationUsed: options.optimization,
        optimizationRuns: parseInt(options.runs, 10),
        constructorArguments: options.constructorArgs || '',
        etherscanApiKey,
      });

      console.log(`\nVerification result:`);
      console.log(`  Verified: ${result.verified}`);

      if (result.results.etherscan) {
        console.log(`  Etherscan: ${result.results.etherscan.verified ? 'Verified' : result.results.etherscan.error || 'Not verified'}`);
      }
      if (result.results.sourcify) {
        console.log(`  Sourcify: ${result.results.sourcify.verified ? `Verified (${result.results.sourcify.matchType})` : result.results.sourcify.error || 'Not verified'}`);
      }
      if (result.url) {
        console.log(`  Explorer: ${result.url}`);
      }
    });
}
