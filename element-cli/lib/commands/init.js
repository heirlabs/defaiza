import { writeFileSync, mkdirSync, existsSync } from 'fs';
import { resolve, join } from 'path';

const CONTRACT_CONFIG_TEMPLATE = {
  network: 'polygon',
  ownerAddress: '',
  beneficiaries: [
    {
      name: 'Primary Beneficiary',
      address: '',
      percentage: 60,
      relationship: 'spouse',
    },
    {
      name: 'Secondary Beneficiary',
      address: '',
      percentage: 40,
      relationship: 'child',
    },
  ],
  assets: [
    {
      type: 'native',
      symbol: 'MATIC',
      amount: '0',
    },
  ],
  inheritanceTemplate: {
    type: 'common-law',
    params: {},
  },
  deadMansSwitch: {
    type: 'timeout',
    lockupDays: 365,
    graceDays: 30,
  },
  useOracle: false,
  jurisdiction: null,
  ownership: {
    mode: 'single',
  },
};

const ENV_TEMPLATE = `# HEIR.ES DEFAI Element SDK Configuration
# Fill in the values for your deployment environment

# RPC URLs (at least one required for deployment)
ETHEREUM_RPC_URL=
POLYGON_RPC_URL=
BASE_RPC_URL=
AVALANCHE_RPC_URL=
ARBITRUM_RPC_URL=
OPTIMISM_RPC_URL=
SOLANA_RPC_URL=

# Deployer private key (KEEP SECRET - never commit this file)
DEPLOYER_PRIVATE_KEY=

# Block explorer API keys (for contract verification)
ETHERSCAN_API_KEY=
POLYGONSCAN_API_KEY=
BASESCAN_API_KEY=

# HEIR.ES API (optional - for remote contract generation)
HEIR_API_URL=https://api.heir.es
`;

const GITIGNORE_TEMPLATE = `.env
node_modules/
dist/
artifacts/
cache/
*.key
`;

export function registerInitCommand(program) {
  program
    .command('init [directory]')
    .description('Initialize a new HEIR.ES project with SDK configuration')
    .option('--network <network>', 'Default network', 'polygon')
    .option('--template <type>', 'Inheritance template type', 'common-law')
    .option('--force', 'Overwrite existing files')
    .action(async (directory, options) => {
      const projectDir = resolve(directory || '.');

      if (!existsSync(projectDir)) {
        mkdirSync(projectDir, { recursive: true });
      }

      const contractsDir = join(projectDir, 'contracts');
      if (!existsSync(contractsDir)) {
        mkdirSync(contractsDir, { recursive: true });
      }

      const configPath = join(projectDir, 'heir.config.json');
      if (existsSync(configPath) && !options.force) {
        console.error(`Configuration already exists: ${configPath}. Use --force to overwrite.`);
        process.exit(1);
      }

      const config = {
        ...CONTRACT_CONFIG_TEMPLATE,
        network: options.network,
        inheritanceTemplate: { type: options.template, params: {} },
      };

      writeFileSync(configPath, JSON.stringify(config, null, 2));
      console.log(`Created: ${configPath}`);

      const envPath = join(projectDir, '.env.heir');
      if (!existsSync(envPath) || options.force) {
        writeFileSync(envPath, ENV_TEMPLATE);
        console.log(`Created: ${envPath}`);
      }

      const gitignorePath = join(projectDir, '.gitignore');
      if (!existsSync(gitignorePath) || options.force) {
        writeFileSync(gitignorePath, GITIGNORE_TEMPLATE);
        console.log(`Created: ${gitignorePath}`);
      }

      console.log('\nHEIR.ES project initialized successfully!');
      console.log('\nNext steps:');
      console.log('  1. Edit heir.config.json with your contract parameters');
      console.log('  2. Set your wallet addresses for owner and beneficiaries');
      console.log('  3. Configure RPC URLs in .env.heir');
      console.log('  4. Generate contract:  npx defai generate -c heir.config.json -o contracts/Inheritance.sol');
      console.log('  5. Estimate gas:       npx defai estimate -c heir.config.json');
      console.log('  6. Deploy (after compilation): npx defai deploy -b <bytecode> -n polygon');
    });
}
