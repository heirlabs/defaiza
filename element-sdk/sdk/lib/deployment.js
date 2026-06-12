import { SDK_EVENTS } from './event-emitter.js';
import { isSolanaChain } from './contract-generator.js';

const EVM_CHAIN_CONFIG = {
  ethereum: { chainId: 1, name: 'Ethereum Mainnet', rpcEnv: 'VITE_ETHEREUM_RPC_URL', explorer: 'https://etherscan.io' },
  polygon: { chainId: 137, name: 'Polygon', rpcEnv: 'VITE_POLYGON_RPC_URL', explorer: 'https://polygonscan.com' },
  base: { chainId: 8453, name: 'Base', rpcEnv: 'VITE_BASE_RPC_URL', explorer: 'https://basescan.org' },
  avalanche: { chainId: 43114, name: 'Avalanche C-Chain', rpcEnv: 'VITE_AVALANCHE_RPC_URL', explorer: 'https://snowtrace.io' },
  arbitrum: { chainId: 42161, name: 'Arbitrum One', rpcEnv: 'VITE_ARBITRUM_RPC_URL', explorer: 'https://arbiscan.io' },
  optimism: { chainId: 10, name: 'Optimism', rpcEnv: 'VITE_OPTIMISM_RPC_URL', explorer: 'https://optimistic.etherscan.io' },
};

const SOLANA_CLUSTER_CONFIG = {
  solana: { cluster: 'mainnet-beta', rpcEnv: 'VITE_SOLANA_RPC_URL' },
  'solana-devnet': { cluster: 'devnet', rpcEnv: 'VITE_SOLANA_DEVNET_RPC_URL' },
  'solana-testnet': { cluster: 'testnet', rpcEnv: 'VITE_SOLANA_TESTNET_RPC_URL' },
};

function getEnv(key) {
  if (typeof import.meta !== 'undefined' && import.meta.env) {
    return import.meta.env[key];
  }
  if (typeof process !== 'undefined' && process.env) {
    return process.env[key];
  }
  return undefined;
}

export class ContractDeployer {
  constructor(emitter) {
    this._emitter = emitter;
    this._deploymentCount = 0;
  }

  get deploymentCount() {
    return this._deploymentCount;
  }

  async deploy(contractCode, options) {
    const { network, constructorArgs = [], walletClient, signer, bytecode, abi } = options;

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_START, { network });

    if (isSolanaChain(network)) {
      return this._deploySolana(contractCode, options);
    }

    return this._deployEVM(contractCode, options);
  }

  async _deployEVM(contractCode, options) {
    const { network, constructorArgs = [], walletClient, signer, bytecode, abi } = options;
    const chainConfig = EVM_CHAIN_CONFIG[network];
    if (!chainConfig) {
      throw new Error(`Unsupported EVM network: ${network}. Supported: ${Object.keys(EVM_CHAIN_CONFIG).join(', ')}`);
    }

    const deployBytecode = bytecode || options.compiledBytecode;
    if (!deployBytecode) {
      throw new Error('Bytecode is required for EVM deployment. Compile the contract first with solc or Hardhat.');
    }

    if (walletClient) {
      return this._deployWithViem(walletClient, deployBytecode, abi, constructorArgs, chainConfig);
    }

    if (signer) {
      return this._deployWithEthers(signer, deployBytecode, abi, constructorArgs, chainConfig);
    }

    throw new Error('Either walletClient (viem) or signer (ethers) is required for EVM deployment.');
  }

  async _deployWithViem(walletClient, bytecode, abi, constructorArgs, chainConfig) {
    const { createPublicClient, http, encodeDeployData } = await import('viem');

    const rpcUrl = getEnv(chainConfig.rpcEnv);
    const publicClient = createPublicClient({
      chain: { id: chainConfig.chainId, name: chainConfig.name },
      transport: http(rpcUrl),
    });

    const deployData = encodeDeployData({ abi: abi || [], bytecode, args: constructorArgs });

    const txHash = await walletClient.deployContract({ abi: abi || [], bytecode, args: constructorArgs });

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_TX_SENT, {
      txHash,
      chainId: chainConfig.chainId,
      network: chainConfig.name,
    });

    const receipt = await publicClient.waitForTransactionReceipt({ hash: txHash });

    this._deploymentCount++;

    const result = {
      address: receipt.contractAddress,
      txHash: receipt.transactionHash,
      chainId: chainConfig.chainId,
      network: chainConfig.name,
      blockNumber: Number(receipt.blockNumber),
      gasUsed: Number(receipt.gasUsed),
      explorerUrl: `${chainConfig.explorer}/address/${receipt.contractAddress}`,
      deployedAt: Date.now(),
    };

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_CONFIRMED, result);
    return result;
  }

  async _deployWithEthers(signer, bytecode, abi, constructorArgs, chainConfig) {
    const { ethers } = await import('ethers');

    const factory = new ethers.ContractFactory(abi || [], bytecode, signer);
    const contract = await factory.deploy(...constructorArgs);

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_TX_SENT, {
      txHash: contract.deploymentTransaction()?.hash,
      chainId: chainConfig.chainId,
      network: chainConfig.name,
    });

    await contract.waitForDeployment();
    const address = await contract.getAddress();
    const deployTx = contract.deploymentTransaction();

    this._deploymentCount++;

    const result = {
      address,
      txHash: deployTx?.hash,
      chainId: chainConfig.chainId,
      network: chainConfig.name,
      blockNumber: deployTx?.blockNumber || null,
      gasUsed: null,
      explorerUrl: `${chainConfig.explorer}/address/${address}`,
      deployedAt: Date.now(),
    };

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_CONFIRMED, result);
    return result;
  }

  async _deploySolana(contractCode, options) {
    const { network = 'solana', keypair, programKeypair } = options;
    const clusterConfig = SOLANA_CLUSTER_CONFIG[network] || SOLANA_CLUSTER_CONFIG['solana'];

    if (!keypair) {
      throw new Error('Solana keypair (payer) is required for deployment.');
    }
    if (!programKeypair) {
      throw new Error('Solana programKeypair is required for deployment. Generate with: solana-keygen new');
    }

    const { Connection, BpfLoader, BPF_LOADER_PROGRAM_ID } = await import('@solana/web3.js');

    const rpcUrl = getEnv(clusterConfig.rpcEnv) || `https://api.${clusterConfig.cluster}.solana.com`;
    const connection = new Connection(rpcUrl, 'confirmed');

    const programBinary = options.programBinary;
    if (!programBinary) {
      throw new Error('Compiled program binary (Buffer) is required for Solana deployment. Build with: anchor build');
    }

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_TX_SENT, {
      network: clusterConfig.cluster,
      programId: programKeypair.publicKey.toBase58(),
    });

    await BpfLoader.load(connection, keypair, programKeypair, programBinary, BPF_LOADER_PROGRAM_ID);

    this._deploymentCount++;

    const result = {
      address: programKeypair.publicKey.toBase58(),
      txHash: null,
      chainId: clusterConfig.cluster,
      network: clusterConfig.cluster,
      blockNumber: null,
      gasUsed: null,
      explorerUrl: `https://explorer.solana.com/address/${programKeypair.publicKey.toBase58()}?cluster=${clusterConfig.cluster}`,
      deployedAt: Date.now(),
    };

    this._emitter.emit(SDK_EVENTS.DEPLOYMENT_CONFIRMED, result);
    return result;
  }
}

export { EVM_CHAIN_CONFIG, SOLANA_CLUSTER_CONFIG };
