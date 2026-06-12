import type EventEmitter from 'eventemitter3';

// ---- Enums & Constants ----

export declare const SDK_VERSION: string;

export declare const SDK_EVENTS: {
  readonly CONTRACT_GENERATION_START: 'contract:generation:start';
  readonly CONTRACT_GENERATION_COMPLETE: 'contract:generation:complete';
  readonly CONTRACT_GENERATION_ERROR: 'contract:generation:error';
  readonly DEPLOYMENT_START: 'deployment:start';
  readonly DEPLOYMENT_TX_SENT: 'deployment:tx:sent';
  readonly DEPLOYMENT_CONFIRMED: 'deployment:confirmed';
  readonly DEPLOYMENT_ERROR: 'deployment:error';
  readonly VERIFICATION_START: 'verification:start';
  readonly VERIFICATION_COMPLETE: 'verification:complete';
  readonly VERIFICATION_ERROR: 'verification:error';
  readonly GAS_ESTIMATION_START: 'gas:estimation:start';
  readonly GAS_ESTIMATION_COMPLETE: 'gas:estimation:complete';
  readonly GAS_ESTIMATION_ERROR: 'gas:estimation:error';
  readonly SDK_INITIALIZED: 'sdk:initialized';
  readonly SDK_ERROR: 'sdk:error';
};

export declare const SUPPORTED_BLOCKCHAINS: {
  evm: string[];
  solana: string[];
};

export declare const EVM_CHAIN_CONFIG: Record<string, EVMChainConfig>;
export declare const SOLANA_CLUSTER_CONFIG: Record<string, SolanaClusterConfig>;

// ---- Interfaces ----

export interface EVMChainConfig {
  chainId: number;
  name: string;
  rpcEnv: string;
  explorer: string;
}

export interface SolanaClusterConfig {
  cluster: string;
  rpcEnv: string;
}

export interface Beneficiary {
  name: string;
  address: string;
  percentage: number;
  relationship?: string;
  email?: string;
}

export interface Asset {
  type: 'native' | 'token' | 'nft';
  address?: string;
  amount?: string;
  symbol?: string;
  decimals?: number;
  tokenId?: string;
}

export interface InheritanceTemplate {
  type: string;
  params?: Record<string, unknown>;
  combinedMode?: boolean;
  primary?: { type: string; allocation: number };
  secondary?: { type: string; allocation: number };
}

export interface DeadMansSwitch {
  type?: 'timeout' | 'oracle';
  lockupPeriod?: number;
  lockupDays?: number;
  gracePeriod?: number;
  graceDays?: number;
}

export interface Ownership {
  mode: 'single' | 'multisig';
  signers?: string[];
  threshold?: number;
}

export interface ContractConfig {
  network: string;
  ownerAddress: string;
  beneficiaries: Beneficiary[];
  assets?: Asset[];
  inheritanceTemplate?: InheritanceTemplate;
  deadMansSwitch?: DeadMansSwitch;
  useOracle?: boolean;
  jurisdiction?: string;
  ownership?: Ownership;
  networks?: string[];
}

export interface GenerationResult {
  code: string;
  abi: ABIEntry[] | null;
  metadata: ContractMetadata;
}

export interface ContractMetadata {
  blockchain: string;
  network: string;
  generatedAt: number;
  generationTimeMs: number;
  templateType: string;
  beneficiaryCount: number;
  ownership: Ownership;
  version: string;
  [key: string]: unknown;
}

export interface ABIEntry {
  type: string;
  name?: string;
  inputs?: ABIParam[];
  outputs?: ABIParam[];
  stateMutability?: string;
}

export interface ABIParam {
  name: string;
  type: string;
  indexed?: boolean;
}

export interface DeploymentOptions {
  network: string;
  constructorArgs?: unknown[];
  walletClient?: unknown;
  signer?: unknown;
  bytecode?: string;
  compiledBytecode?: string;
  abi?: ABIEntry[];
  keypair?: unknown;
  programKeypair?: unknown;
  programBinary?: Buffer;
}

export interface DeploymentResult {
  address: string;
  txHash: string | null;
  chainId: number | string;
  network: string;
  blockNumber: number | null;
  gasUsed: number | null;
  explorerUrl: string;
  deployedAt: number;
}

export interface VerificationOptions {
  sourceCode?: string;
  contractName?: string;
  compilerVersion?: string;
  optimizationUsed?: boolean;
  optimizationRuns?: number;
  constructorArguments?: string;
  evmVersion?: string;
  metadata?: string | Record<string, unknown>;
  etherscanApiKey?: string;
}

export interface VerificationResult {
  verified: boolean;
  url: string | null;
  address: string;
  chainId: number;
  network: string;
  results: {
    etherscan: { verified: boolean; guid?: string; error?: string } | null;
    sourcify: { verified: boolean; matchType?: string; url?: string; error?: string } | null;
  };
  verifiedAt: number | null;
}

export interface GasEstimationResult {
  gasLimit: string | null;
  gasPrice: string | null;
  totalCost: string | null;
  totalCostUSD: string | null;
  chainCosts: Record<string, ChainGasEstimate>;
  estimatedAt: number;
}

export interface ChainGasEstimate {
  network: string;
  chainId: number | string;
  chainName: string;
  gasLimit: string | null;
  gasPrice: string | null;
  gasPriceGwei?: string;
  totalCostWei?: string;
  totalCostLamports?: string;
  totalCostNative: string;
  totalCostUSD: string;
  nativeToken: string;
  nativePriceUSD: number;
  breakdown: Record<string, { gas?: string; lamports?: string; costNative?: string; costSOL?: string; costUSD?: string }>;
  error?: string;
}

export interface EventLogEntry {
  event: string;
  timestamp: number;
  data: unknown;
}

export interface EventLogFilter {
  event?: string;
  since?: number;
  until?: number;
}

export interface SDKMetrics {
  generationCount: number;
  deploymentCount: number;
  verificationCount: number;
  estimationCount: number;
  uptimeMs: number;
  eventLogSize: number;
}

export interface SDKOptions {
  apiUrl?: string;
}

export interface ElementState {
  [key: string]: unknown;
}

// ---- Classes ----

export declare class ElementEventEmitter extends EventEmitter {
  getEventLog(filter?: EventLogFilter): EventLogEntry[];
  clearEventLog(): void;
}

export declare class DefaiElement {
  currentContext: unknown;
  setState(partial: Partial<ElementState>): void;
  getState(): ElementState;
  saveData(key: string, data: unknown): Promise<void>;
  loadData(key: string): Promise<unknown>;
  emitToOthers(event: string, data: unknown): void;
  onFromOthers(event: string, handler: (data: unknown) => void): void;
  onMount(context: unknown): Promise<void>;
  onUnmount(): Promise<void>;
  onResize(size: { width: number; height: number }): Promise<void>;
  onSettingsChange(settings: Record<string, unknown>): Promise<void>;
}

export declare class ElementSDK {
  constructor(options?: SDKOptions);
  readonly version: string;
  readonly events: ElementEventEmitter;
  readonly metrics: SDKMetrics;
  on(event: string, handler: (...args: unknown[]) => void): this;
  off(event: string, handler: (...args: unknown[]) => void): this;
  generateContract(config: ContractConfig): Promise<GenerationResult>;
  deployContract(code: string, options: DeploymentOptions): Promise<DeploymentResult>;
  verifyContract(address: string, chainId: number, options?: VerificationOptions): Promise<VerificationResult>;
  estimateGas(config: ContractConfig): Promise<GasEstimationResult>;
  checkVerificationStatus(address: string, chainId: number): Promise<VerificationResult>;
  getEventLog(filter?: EventLogFilter): EventLogEntry[];
}

export declare class ContractGenerator {
  constructor(emitter: ElementEventEmitter, options?: { apiUrl?: string });
  readonly generationCount: number;
  generate(config: ContractConfig): Promise<GenerationResult>;
}

export declare class ContractDeployer {
  constructor(emitter: ElementEventEmitter);
  readonly deploymentCount: number;
  deploy(contractCode: string, options: DeploymentOptions): Promise<DeploymentResult>;
}

export declare class ContractVerifier {
  constructor(emitter: ElementEventEmitter);
  readonly verificationCount: number;
  verify(address: string, chainId: number, options?: VerificationOptions): Promise<VerificationResult>;
  checkVerificationStatus(address: string, chainId: number): Promise<VerificationResult>;
}

export declare class GasEstimator {
  constructor(emitter: ElementEventEmitter);
  readonly estimationCount: number;
  estimate(config: ContractConfig): Promise<GasEstimationResult>;
}

// ---- Factory Functions ----

export declare function createElementSDK(options?: SDKOptions): ElementSDK;
export declare function isSolanaChain(network: string): boolean;
export declare function isEVMChain(network: string): boolean;

// ---- React Bindings (from @defai/element-react) ----

export declare function ElementProvider(props: {
  children: React.ReactNode;
  apiUrl?: string;
  options?: SDKOptions;
}): React.ReactElement;

export declare function useElementSDK(): ElementSDK;

export declare function useContractGeneration(): {
  generate: (config: ContractConfig) => Promise<GenerationResult>;
  result: GenerationResult | null;
  loading: boolean;
  error: Error | null;
  reset: () => void;
};

export declare function useDeployment(): {
  deploy: (code: string, options: DeploymentOptions) => Promise<DeploymentResult>;
  result: DeploymentResult | null;
  loading: boolean;
  error: Error | null;
  txHash: string | null;
  reset: () => void;
};

export declare function useGasEstimation(): {
  estimate: (config: ContractConfig) => Promise<GasEstimationResult>;
  result: GasEstimationResult | null;
  loading: boolean;
  error: Error | null;
  reset: () => void;
};

export declare function useContractVerification(): {
  verify: (address: string, chainId: number, options?: VerificationOptions) => Promise<VerificationResult>;
  checkStatus: (address: string, chainId: number) => Promise<VerificationResult>;
  result: VerificationResult | null;
  loading: boolean;
  error: Error | null;
  reset: () => void;
};

export declare function useSDKEvents(eventName: string, handler: (...args: unknown[]) => void): void;

export declare function useSDKMetrics(pollIntervalMs?: number): SDKMetrics;
