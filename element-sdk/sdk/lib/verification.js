import { SDK_EVENTS } from './event-emitter.js';
import { EVM_CHAIN_CONFIG } from './deployment.js';

const ETHERSCAN_API_KEYS_ENV = {
  ethereum: 'ETHERSCAN_API_KEY',
  polygon: 'POLYGONSCAN_API_KEY',
  base: 'BASESCAN_API_KEY',
  avalanche: 'SNOWTRACE_API_KEY',
  arbitrum: 'ARBISCAN_API_KEY',
  optimism: 'OPTIMISTIC_ETHERSCAN_API_KEY',
};

const ETHERSCAN_API_URLS = {
  ethereum: 'https://api.etherscan.io/api',
  polygon: 'https://api.polygonscan.com/api',
  base: 'https://api.basescan.org/api',
  avalanche: 'https://api.snowtrace.io/api',
  arbitrum: 'https://api.arbiscan.io/api',
  optimism: 'https://api-optimistic.etherscan.io/api',
};

const SOURCIFY_API = 'https://sourcify.dev/server';

function getEnv(key) {
  if (typeof import.meta !== 'undefined' && import.meta.env) {
    return import.meta.env[key];
  }
  if (typeof process !== 'undefined' && process.env) {
    return process.env[key];
  }
  return undefined;
}

export class ContractVerifier {
  constructor(emitter) {
    this._emitter = emitter;
    this._verificationCount = 0;
  }

  get verificationCount() {
    return this._verificationCount;
  }

  async verify(address, chainId, options = {}) {
    this._emitter.emit(SDK_EVENTS.VERIFICATION_START, { address, chainId });

    const network = this._getNetworkByChainId(chainId);
    if (!network) {
      throw new Error(`Unsupported chain ID for verification: ${chainId}`);
    }

    const results = { etherscan: null, sourcify: null };

    const etherscanApiKey = options.etherscanApiKey || getEnv(ETHERSCAN_API_KEYS_ENV[network]);
    if (etherscanApiKey && options.sourceCode) {
      results.etherscan = await this._verifyEtherscan(address, network, etherscanApiKey, options);
    }

    if (options.sourceCode) {
      results.sourcify = await this._verifySourcify(address, chainId, options);
    }

    const verified = !!(results.etherscan?.verified || results.sourcify?.verified);
    this._verificationCount++;

    const explorerUrl = EVM_CHAIN_CONFIG[network]
      ? `${EVM_CHAIN_CONFIG[network].explorer}/address/${address}#code`
      : null;

    const output = {
      verified,
      url: explorerUrl,
      address,
      chainId,
      network,
      results,
      verifiedAt: verified ? Date.now() : null,
    };

    this._emitter.emit(SDK_EVENTS.VERIFICATION_COMPLETE, output);
    return output;
  }

  async checkVerificationStatus(address, chainId) {
    const network = this._getNetworkByChainId(chainId);
    if (!network) {
      return { verified: false, network: null };
    }

    const sourcifyStatus = await this._checkSourcifyStatus(address, chainId);
    const explorerUrl = EVM_CHAIN_CONFIG[network]
      ? `${EVM_CHAIN_CONFIG[network].explorer}/address/${address}#code`
      : null;

    return {
      verified: sourcifyStatus.verified,
      url: explorerUrl,
      address,
      chainId,
      network,
      sourcify: sourcifyStatus,
    };
  }

  async _verifyEtherscan(address, network, apiKey, options) {
    const apiUrl = ETHERSCAN_API_URLS[network];
    if (!apiUrl) return { verified: false, error: 'No Etherscan API URL for network' };

    const params = new URLSearchParams({
      module: 'contract',
      action: 'verifysourcecode',
      apikey: apiKey,
      contractaddress: address,
      sourceCode: options.sourceCode,
      codeformat: 'solidity-single-file',
      contractname: options.contractName || 'InheritanceContract',
      compilerversion: options.compilerVersion || 'v0.8.24+commit.e11b9ed9',
      optimizationUsed: options.optimizationUsed ? '1' : '0',
      runs: String(options.optimizationRuns || 200),
      // NOTE: "constructorArguements" is intentionally misspelled — it is the
      // exact (historically misspelled) parameter name required by the
      // Etherscan verifysourcecode API. Do not "correct" it.
      constructorArguements: options.constructorArguments || '',
      evmversion: options.evmVersion || 'paris',
      licenseType: '3',
    });

    const response = await fetch(apiUrl, { method: 'POST', body: params });
    const data = await response.json();

    if (data.status === '1' && data.result) {
      const guid = data.result;
      return this._pollEtherscanVerification(apiUrl, apiKey, guid);
    }

    return { verified: false, error: data.result || 'Verification submission failed' };
  }

  async _pollEtherscanVerification(apiUrl, apiKey, guid) {
    const maxAttempts = 10;
    const delayMs = 3000;

    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      await new Promise(r => setTimeout(r, delayMs));

      const checkParams = new URLSearchParams({
        module: 'contract',
        action: 'checkverifystatus',
        apikey: apiKey,
        guid,
      });

      const response = await fetch(`${apiUrl}?${checkParams}`);
      const data = await response.json();

      if (data.result === 'Pass - Verified') {
        return { verified: true, guid };
      }
      if (data.result && !data.result.includes('Pending')) {
        return { verified: false, error: data.result };
      }
    }

    return { verified: false, error: 'Verification timed out' };
  }

  async _verifySourcify(address, chainId, options) {
    const body = {
      address,
      chain: String(chainId),
      files: {
        'InheritanceContract.sol': options.sourceCode,
      },
    };

    if (options.metadata) {
      body.files['metadata.json'] = typeof options.metadata === 'string'
        ? options.metadata
        : JSON.stringify(options.metadata);
    }

    const response = await fetch(`${SOURCIFY_API}/verify`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      const errorText = await response.text();
      return { verified: false, error: `Sourcify returned ${response.status}: ${errorText}` };
    }

    const data = await response.json();
    const match = data.result?.[0];

    return {
      verified: match?.status === 'perfect' || match?.status === 'partial',
      matchType: match?.status || null,
      url: `${SOURCIFY_API}/files/any/${chainId}/${address}`,
    };
  }

  async _checkSourcifyStatus(address, chainId) {
    const response = await fetch(`${SOURCIFY_API}/check-by-addresses?addresses=${address}&chainIds=${chainId}`);

    if (!response.ok) {
      return { verified: false };
    }

    const data = await response.json();
    const entry = data?.[0];

    if (!entry || !entry.chainIds) {
      return { verified: false };
    }

    const chainEntry = entry.chainIds.find(c => String(c.chainId) === String(chainId));
    return {
      verified: chainEntry?.status === 'perfect' || chainEntry?.status === 'partial',
      matchType: chainEntry?.status || null,
    };
  }

  _getNetworkByChainId(chainId) {
    for (const [network, config] of Object.entries(EVM_CHAIN_CONFIG)) {
      if (config.chainId === chainId) return network;
    }
    return null;
  }
}
