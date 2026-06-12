import { SDK_EVENTS } from './event-emitter.js';
import { EVM_CHAIN_CONFIG, SOLANA_CLUSTER_CONFIG } from './deployment.js';
import { isSolanaChain } from './contract-generator.js';

const COINGECKO_PRICE_API = 'https://api.coingecko.com/api/v3/simple/price';

const CHAIN_NATIVE_TOKENS = {
  ethereum: { id: 'ethereum', symbol: 'ETH', decimals: 18 },
  polygon: { id: 'matic-network', symbol: 'MATIC', decimals: 18 },
  base: { id: 'ethereum', symbol: 'ETH', decimals: 18 },
  avalanche: { id: 'avalanche-2', symbol: 'AVAX', decimals: 18 },
  arbitrum: { id: 'ethereum', symbol: 'ETH', decimals: 18 },
  optimism: { id: 'ethereum', symbol: 'ETH', decimals: 18 },
  solana: { id: 'solana', symbol: 'SOL', decimals: 9 },
};

const INHERITANCE_CONTRACT_GAS_ESTIMATES = {
  deployment: 2_500_000n,
  claim: 80_000n,
  claimToken: 95_000n,
  resetTimer: 35_000n,
  updateBeneficiaries: 150_000n,
  setModule: 120_000n,
  claimModule: 100_000n,
  terminateContract: 60_000n,
};

const SOLANA_COST_ESTIMATES = {
  deploymentLamports: 5_000_000_000n,
  createEstateLamports: 10_000_000n,
  claimInheritanceLamports: 5_000n,
  checkInLamports: 5_000n,
  updateBeneficiariesLamports: 5_000n,
  closeEstateLamports: 5_000n,
  rentExemptionLamports: 2_000_000n,
};

function getEnv(key) {
  if (typeof import.meta !== 'undefined' && import.meta.env) return import.meta.env[key];
  if (typeof process !== 'undefined' && process.env) return process.env[key];
  return undefined;
}

export class GasEstimator {
  constructor(emitter) {
    this._emitter = emitter;
    this._estimationCount = 0;
    this._priceCache = new Map();
    this._priceCacheTTL = 60_000;
  }

  get estimationCount() {
    return this._estimationCount;
  }

  async estimate(config) {
    this._emitter.emit(SDK_EVENTS.GAS_ESTIMATION_START, {
      network: config.network,
      beneficiaryCount: config.beneficiaries?.length || 0,
    });

    const networks = config.networks || (config.network ? [config.network] : ['ethereum']);
    const priceIds = [...new Set(networks.map(n => CHAIN_NATIVE_TOKENS[n]?.id).filter(Boolean))];
    const prices = await this._fetchPrices(priceIds);

    const estimatePromises = networks.map(network => this._estimateForNetwork(network, config, prices));
    const chainEstimates = await Promise.all(estimatePromises);

    const chainCosts = {};
    for (const est of chainEstimates) {
      chainCosts[est.network] = est;
    }

    const primaryNetwork = config.network || networks[0];
    const primary = chainCosts[primaryNetwork] || chainEstimates[0];

    this._estimationCount++;

    const result = {
      gasLimit: primary?.gasLimit || null,
      gasPrice: primary?.gasPrice || null,
      totalCost: primary?.totalCostNative || null,
      totalCostUSD: primary?.totalCostUSD || null,
      chainCosts,
      estimatedAt: Date.now(),
    };

    this._emitter.emit(SDK_EVENTS.GAS_ESTIMATION_COMPLETE, {
      network: primaryNetwork,
      totalCostUSD: result.totalCostUSD,
      chainsEstimated: Object.keys(chainCosts).length,
    });

    return result;
  }

  async _estimateForNetwork(network, config, prices) {
    if (isSolanaChain(network)) {
      return this._estimateSolana(network, config, prices);
    }
    return this._estimateEVM(network, config, prices);
  }

  async _estimateEVM(network, config, prices) {
    const chainConfig = EVM_CHAIN_CONFIG[network];
    if (!chainConfig) {
      return { network, error: `Unsupported network: ${network}`, gasLimit: null, gasPrice: null, totalCostNative: null, totalCostUSD: null };
    }

    const tokenInfo = CHAIN_NATIVE_TOKENS[network];
    const priceUSD = prices[tokenInfo.id]?.usd || 0;

    let gasPrice = 30_000_000_000n;
    const rpcUrl = getEnv(chainConfig.rpcEnv);

    if (rpcUrl) {
      const rpcGasPrice = await this._fetchEVMGasPrice(rpcUrl);
      if (rpcGasPrice) gasPrice = rpcGasPrice;
    }

    const beneficiaryCount = BigInt(config.beneficiaries?.length || 1);
    const deployGas = INHERITANCE_CONTRACT_GAS_ESTIMATES.deployment + (beneficiaryCount * 25_000n);

    const totalGas = deployGas;
    const totalWei = totalGas * gasPrice;
    const totalNative = Number(totalWei) / 1e18;
    const totalUSD = totalNative * priceUSD;

    return {
      network,
      chainId: chainConfig.chainId,
      chainName: chainConfig.name,
      gasLimit: totalGas.toString(),
      gasPrice: gasPrice.toString(),
      gasPriceGwei: (Number(gasPrice) / 1e9).toFixed(2),
      totalCostWei: totalWei.toString(),
      totalCostNative: totalNative.toFixed(6),
      totalCostUSD: totalUSD.toFixed(2),
      nativeToken: tokenInfo.symbol,
      nativePriceUSD: priceUSD,
      breakdown: {
        deployment: { gas: deployGas.toString(), costNative: (Number(deployGas * gasPrice) / 1e18).toFixed(6), costUSD: ((Number(deployGas * gasPrice) / 1e18) * priceUSD).toFixed(2) },
        claim: { gas: INHERITANCE_CONTRACT_GAS_ESTIMATES.claim.toString() },
        resetTimer: { gas: INHERITANCE_CONTRACT_GAS_ESTIMATES.resetTimer.toString() },
      },
    };
  }

  async _estimateSolana(network, config, prices) {
    const tokenInfo = CHAIN_NATIVE_TOKENS['solana'];
    const priceUSD = prices[tokenInfo.id]?.usd || 0;

    const totalLamports = SOLANA_COST_ESTIMATES.createEstateLamports
      + SOLANA_COST_ESTIMATES.rentExemptionLamports;
    const totalSOL = Number(totalLamports) / 1e9;
    const totalUSD = totalSOL * priceUSD;

    return {
      network,
      chainId: 'solana',
      chainName: 'Solana',
      gasLimit: null,
      gasPrice: null,
      totalCostLamports: totalLamports.toString(),
      totalCostNative: totalSOL.toFixed(6),
      totalCostUSD: totalUSD.toFixed(2),
      nativeToken: 'SOL',
      nativePriceUSD: priceUSD,
      breakdown: {
        createEstate: { lamports: SOLANA_COST_ESTIMATES.createEstateLamports.toString(), costSOL: (Number(SOLANA_COST_ESTIMATES.createEstateLamports) / 1e9).toFixed(6) },
        rentExemption: { lamports: SOLANA_COST_ESTIMATES.rentExemptionLamports.toString(), costSOL: (Number(SOLANA_COST_ESTIMATES.rentExemptionLamports) / 1e9).toFixed(6) },
        transactionFee: { lamports: '5000', costSOL: '0.000005' },
      },
    };
  }

  async _fetchPrices(coinIds) {
    const now = Date.now();
    const cacheKey = coinIds.sort().join(',');
    const cached = this._priceCache.get(cacheKey);

    if (cached && (now - cached.timestamp) < this._priceCacheTTL) {
      return cached.data;
    }

    const fallbackPrices = {
      ethereum: { usd: 3500 },
      'matic-network': { usd: 0.85 },
      'avalanche-2': { usd: 35 },
      solana: { usd: 140 },
    };

    try {
      const ids = coinIds.join(',');
      const response = await fetch(`${COINGECKO_PRICE_API}?ids=${ids}&vs_currencies=usd`, {
        signal: AbortSignal.timeout(5000),
      });

      if (!response.ok) {
        return fallbackPrices;
      }

      const data = await response.json();
      this._priceCache.set(cacheKey, { data, timestamp: now });
      return data;
    } catch {
      return fallbackPrices;
    }
  }

  async _fetchEVMGasPrice(rpcUrl) {
    try {
      const response = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', method: 'eth_gasPrice', params: [], id: 1 }),
        signal: AbortSignal.timeout(5000),
      });

      if (!response.ok) return null;

      const data = await response.json();
      if (data.result) {
        return BigInt(data.result);
      }
      return null;
    } catch {
      return null;
    }
  }
}
