import { createElementSDK } from '@defai/element-sdk';
import { readFileSync, existsSync } from 'fs';
import { resolve } from 'path';

export function registerEstimateCommand(program) {
  program
    .command('estimate')
    .description('Estimate gas costs for contract deployment across chains')
    .option('-c, --config <path>', 'Path to contract configuration JSON file')
    .option('-n, --networks <networks>', 'Comma-separated list of networks to estimate', 'ethereum,polygon,base,avalanche,arbitrum,optimism,solana')
    .option('--beneficiaries <count>', 'Number of beneficiaries (for estimation without config)', '3')
    .option('--format <format>', 'Output format: table, json', 'table')
    .action(async (options) => {
      let config = {};

      if (options.config) {
        const configPath = resolve(options.config);
        if (!existsSync(configPath)) {
          console.error(`Configuration file not found: ${configPath}`);
          process.exit(1);
        }
        config = JSON.parse(readFileSync(configPath, 'utf-8'));
      } else {
        config = {
          network: 'ethereum',
          ownerAddress: '0x0000000000000000000000000000000000000001',
          beneficiaries: Array.from({ length: parseInt(options.beneficiaries, 10) }, (_, i) => ({
            name: `Beneficiary ${i + 1}`,
            address: `0x${'0'.repeat(39)}${i + 2}`,
            percentage: Math.floor(100 / parseInt(options.beneficiaries, 10)),
          })),
        };
      }

      const networks = options.networks.split(',').map(n => n.trim());
      config.networks = networks;

      const sdk = createElementSDK();

      console.log(`Estimating gas costs for ${networks.length} networks...`);
      console.log(`  Beneficiaries: ${config.beneficiaries?.length || 0}\n`);

      const result = await sdk.estimateGas(config);

      if (options.format === 'json') {
        console.log(JSON.stringify(result, null, 2));
        return;
      }

      const header = ['Network', 'Chain ID', 'Token', 'Gas/Lamports', 'Cost (Native)', 'Cost (USD)'];
      const widths = [15, 10, 8, 18, 16, 12];

      const headerLine = header.map((h, i) => h.padEnd(widths[i])).join(' | ');
      const separator = widths.map(w => '-'.repeat(w)).join('-+-');

      console.log(headerLine);
      console.log(separator);

      for (const [network, estimate] of Object.entries(result.chainCosts)) {
        if (estimate.error) {
          console.log(`${network.padEnd(widths[0])} | ${'ERROR'.padEnd(widths[1])} | ${estimate.error}`);
          continue;
        }

        const row = [
          network.padEnd(widths[0]),
          String(estimate.chainId).padEnd(widths[1]),
          estimate.nativeToken.padEnd(widths[2]),
          (estimate.gasLimit || estimate.totalCostLamports || 'N/A').padEnd(widths[3]),
          `${estimate.totalCostNative} ${estimate.nativeToken}`.padEnd(widths[4]),
          `$${estimate.totalCostUSD}`.padEnd(widths[5]),
        ];
        console.log(row.join(' | '));
      }

      console.log(separator);

      if (result.totalCostUSD) {
        console.log(`\nPrimary network estimate: $${result.totalCostUSD} USD`);
      }
    });
}
