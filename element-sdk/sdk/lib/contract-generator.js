import { SDK_EVENTS } from './event-emitter.js';

const SUPPORTED_BLOCKCHAINS = {
  evm: ['ethereum', 'polygon', 'base', 'avalanche', 'arbitrum', 'optimism'],
  solana: ['solana'],
};

function isSolanaChain(network) {
  return network === 'solana' || network === 'solana-devnet' || network === 'solana-testnet';
}

function isEVMChain(network) {
  return SUPPORTED_BLOCKCHAINS.evm.includes(network);
}

function normalizeConfig(config) {
  const blockchain = isSolanaChain(config.network) ? 'solana' : 'evm';
  const lockupPeriod = config.deadMansSwitch?.lockupPeriod
    || (config.deadMansSwitch?.lockupDays ? config.deadMansSwitch.lockupDays * 86400 : 31536000);
  const gracePeriod = config.deadMansSwitch?.gracePeriod
    || (config.deadMansSwitch?.graceDays ? config.deadMansSwitch.graceDays * 86400 : 2592000);

  return {
    blockchain,
    network: config.network,
    ownerAddress: config.ownerAddress,
    beneficiaries: config.beneficiaries || [],
    assets: config.assets || [],
    inheritanceTemplate: config.inheritanceTemplate || { type: 'common-law', params: {} },
    deadMansSwitch: { lockupPeriod, gracePeriod, type: config.deadMansSwitch?.type || 'timeout' },
    useOracle: config.useOracle || false,
    jurisdiction: config.jurisdiction || null,
    ownership: config.ownership || { mode: 'single' },
  };
}

function generateMinimalABI(config) {
  const isSolana = isSolanaChain(config.network);
  if (isSolana) return null;

  const isMultisig = config.ownership?.mode === 'multisig';
  const abi = [
    { type: 'constructor', inputs: isMultisig
      ? [
          { name: '_lockupPeriod', type: 'uint256' },
          { name: '_owners', type: 'address[]' },
          { name: '_threshold', type: 'uint256' },
        ]
      : [
          { name: '_lockupPeriod', type: 'uint256' },
          { name: '_owner', type: 'address' },
        ],
    },
    { type: 'function', name: 'claim', inputs: [], outputs: [], stateMutability: 'nonpayable' },
    { type: 'function', name: 'claimToken', inputs: [{ name: 'tokenAddress', type: 'address' }], outputs: [], stateMutability: 'nonpayable' },
    { type: 'function', name: 'resetTimer', inputs: [], outputs: [], stateMutability: 'nonpayable' },
    { type: 'function', name: 'isClaimable', inputs: [], outputs: [{ name: '', type: 'bool' }], stateMutability: 'view' },
    { type: 'function', name: 'timeUntilClaimable', inputs: [], outputs: [{ name: '', type: 'uint256' }], stateMutability: 'view' },
    { type: 'function', name: 'setModule', inputs: [
        { name: 'nftContract', type: 'address' },
        { name: 'tokenId', type: 'uint256' },
        { name: 'beneficiary', type: 'address' },
        { name: 'timer', type: 'uint256' },
      ], outputs: [], stateMutability: 'nonpayable' },
    { type: 'function', name: 'claimModule', inputs: [
        { name: 'nftContract', type: 'address' },
        { name: 'tokenId', type: 'uint256' },
        { name: 'beneficiary', type: 'address' },
      ], outputs: [], stateMutability: 'nonpayable' },
    { type: 'function', name: 'terminateContract', inputs: [], outputs: [], stateMutability: 'nonpayable' },
    { type: 'event', name: 'ClaimExecuted', inputs: [
        { name: 'beneficiary', type: 'address', indexed: true },
        { name: 'asset', type: 'address', indexed: true },
        { name: 'amount', type: 'uint256', indexed: false },
      ]},
    { type: 'event', name: 'OwnerActivityUpdated', inputs: [
        { name: 'timestamp', type: 'uint256', indexed: false },
      ]},
    { type: 'event', name: 'ContractTerminated', inputs: [] },
    { type: 'receive', stateMutability: 'payable' },
  ];

  if (isMultisig) {
    abi.push(
      { type: 'function', name: 'proposeResetTimer', inputs: [], outputs: [{ name: '', type: 'uint256' }], stateMutability: 'nonpayable' },
      { type: 'function', name: 'executeResetTimer', inputs: [{ name: 'proposalId', type: 'uint256' }], outputs: [], stateMutability: 'nonpayable' },
      { type: 'function', name: 'confirmProposal', inputs: [{ name: 'proposalId', type: 'uint256' }], outputs: [], stateMutability: 'nonpayable' },
    );
  }

  return abi;
}

export class ContractGenerator {
  constructor(emitter, options = {}) {
    this._emitter = emitter;
    this._apiUrl = options.apiUrl || null;
    this._generationCount = 0;
  }

  get generationCount() {
    return this._generationCount;
  }

  async generate(config) {
    const normalized = normalizeConfig(config);

    this._emitter.emit(SDK_EVENTS.CONTRACT_GENERATION_START, {
      blockchain: normalized.blockchain,
      network: normalized.network,
      beneficiaryCount: normalized.beneficiaries.length,
    });

    const startTime = Date.now();

    if (this._apiUrl) {
      return this._generateViaApi(normalized, startTime);
    }

    return this._generateLocally(normalized, startTime);
  }

  async _generateViaApi(config, startTime) {
    try {
      const response = await fetch(`${this._apiUrl}/api/contracts/generate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
      });

      if (!response.ok) {
        let errorBody;
        try {
          errorBody = await response.text();
        } catch (e) {
          errorBody = 'Failed to read error response';
        }
        
        const error = new Error(`Contract generation API returned ${response.status}: ${errorBody}`);
        this._emitter.emit(SDK_EVENTS.CONTRACT_GENERATION_ERROR, { 
          error: error.message, 
          status: response.status,
          config: { ...config, ownerAddress: '[REDACTED]' } // Don't log sensitive data
        });
        throw error;
      }
    } catch (fetchError) {
      if (fetchError.name === 'TypeError' && fetchError.message.includes('fetch')) {
        const networkError = new Error('Network error: Unable to connect to contract generation API');
        this._emitter.emit(SDK_EVENTS.CONTRACT_GENERATION_ERROR, { 
          error: networkError.message,
          originalError: fetchError.message
        });
        throw networkError;
      }
      throw fetchError; // Re-throw other errors
    }

    const result = await response.json();
    this._generationCount++;

    const output = {
      code: result.code || result.contract?.code,
      abi: result.abi || generateMinimalABI(config),
      metadata: {
        blockchain: config.blockchain,
        network: config.network,
        generatedAt: Date.now(),
        generationTimeMs: Date.now() - startTime,
        templateType: config.inheritanceTemplate.type,
        beneficiaryCount: config.beneficiaries.length,
        ownership: config.ownership,
        version: '1.0.0',
        ...(result.info || {}),
      },
    };

    this._emitter.emit(SDK_EVENTS.CONTRACT_GENERATION_COMPLETE, {
      blockchain: config.blockchain,
      network: config.network,
      codeLength: output.code?.length || 0,
      durationMs: output.metadata.generationTimeMs,
    });

    return output;
  }

  async _generateLocally(config, startTime) {
    // Local generation not available in browser environment
    // Fall back to API generation if available
    if (this._apiUrl) {
      return this._generateViaApi(config, startTime);
    }
    
    // Throw error if no API URL available for local generation
    const error = new Error('Local contract generation not available in browser environment. API URL required.');
    this._emitter.emit(SDK_EVENTS.CONTRACT_GENERATION_ERROR, { error: error.message });
    throw error;
  }
}

export { SUPPORTED_BLOCKCHAINS, isSolanaChain, isEVMChain, normalizeConfig };
