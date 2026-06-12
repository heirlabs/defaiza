import { createElementSDK } from '@defai/element-sdk';
import { readFileSync, writeFileSync, existsSync } from 'fs';
import { resolve, basename } from 'path';

export function registerGenerateCommand(program) {
  program
    .command('generate')
    .description('Generate an inheritance smart contract')
    .requiredOption('-c, --config <path>', 'Path to contract configuration JSON file')
    .option('-o, --output <path>', 'Output file path for generated contract')
    .option('--api-url <url>', 'API URL for remote generation (uses local generators if omitted)')
    .option('--format <format>', 'Output format: solidity, rust, json', 'solidity')
    .action(async (options) => {
      const configPath = resolve(options.config);
      if (!existsSync(configPath)) {
        console.error(`Configuration file not found: ${configPath}`);
        process.exit(1);
      }

      const configRaw = readFileSync(configPath, 'utf-8');
      let config;
      try {
        config = JSON.parse(configRaw);
      } catch {
        console.error(`Invalid JSON in configuration file: ${configPath}`);
        process.exit(1);
      }

      const sdk = createElementSDK({ apiUrl: options.apiUrl || null });

      console.log(`Generating contract for ${config.network || 'ethereum'}...`);
      console.log(`  Owner: ${config.ownerAddress}`);
      console.log(`  Beneficiaries: ${config.beneficiaries?.length || 0}`);
      console.log(`  Template: ${config.inheritanceTemplate?.type || 'common-law'}`);

      const result = await sdk.generateContract(config);

      if (options.output) {
        const outputPath = resolve(options.output);
        if (options.format === 'json') {
          writeFileSync(outputPath, JSON.stringify(result, null, 2));
        } else {
          writeFileSync(outputPath, result.code);
        }
        console.log(`\nContract written to ${outputPath}`);
      } else {
        console.log('\n--- Generated Contract ---\n');
        console.log(result.code);
      }

      console.log('\n--- Metadata ---');
      console.log(`  Blockchain: ${result.metadata.blockchain}`);
      console.log(`  Network: ${result.metadata.network}`);
      console.log(`  Template: ${result.metadata.templateType}`);
      console.log(`  Beneficiaries: ${result.metadata.beneficiaryCount}`);
      console.log(`  Generated in: ${result.metadata.generationTimeMs}ms`);

      if (result.abi) {
        const abiPath = options.output
          ? resolve(options.output.replace(/\.\w+$/, '.abi.json'))
          : null;
        if (abiPath) {
          writeFileSync(abiPath, JSON.stringify(result.abi, null, 2));
          console.log(`  ABI written to ${abiPath}`);
        }
      }
    });
}
