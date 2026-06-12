import { ElementEventEmitter, SDK_EVENTS } from './event-emitter.js';
import { ContractGenerator, SUPPORTED_BLOCKCHAINS, isSolanaChain, isEVMChain } from './contract-generator.js';
import { ContractDeployer, EVM_CHAIN_CONFIG, SOLANA_CLUSTER_CONFIG } from './deployment.js';
import { ContractVerifier } from './verification.js';
import { GasEstimator } from './gas-estimator.js';

const SDK_VERSION = '1.0.0';

export class DefaiElement {
  constructor() {
    this._state = {};
    this.currentContext = null;
    this._messageHandlers = new Map();
  }

  setState(partial) {
    this._state = { ...this._state, ...partial };
  }

  getState() {
    return { ...this._state };
  }

  async saveData(key, data) {
    if (this.currentContext?.api?.saveData) {
      try {
        return await this.currentContext.api.saveData(key, data);
      } catch (error) {
        console.error('API saveData failed:', error);
        // Fallback to localStorage if API fails
      }
    }
    
    if (typeof localStorage !== 'undefined') {
      try {
        const serialized = JSON.stringify(data);
        localStorage.setItem(`defai:${key}`, serialized);
        return true;
      } catch (error) {
        if (error.name === 'QuotaExceededError' || error.code === 22) {
          console.error('Storage quota exceeded, attempting cleanup');
          this._cleanupOldStorage();
          try {
            localStorage.setItem(`defai:${key}`, JSON.stringify(data));
            return true;
          } catch (retryError) {
            console.error('Storage failed even after cleanup:', retryError);
            return false;
          }
        } else if (error.name === 'SecurityError') {
          console.error('Storage blocked by security policy');
          return false;
        } else {
          console.error('Storage serialization failed:', error);
          return false;
        }
      }
    }
    return false;
  }

  async loadData(key) {
    if (this.currentContext?.api?.loadData) {
      try {
        return await this.currentContext.api.loadData(key);
      } catch (error) {
        console.error('API loadData failed:', error);
        // Fallback to localStorage if API fails
      }
    }
    
    if (typeof localStorage !== 'undefined') {
      try {
        const raw = localStorage.getItem(`defai:${key}`);
        if (!raw) return null;
        
        return JSON.parse(raw);
      } catch (error) {
        console.error('Storage deserialization failed for key:', key, error);
        // Remove corrupted data
        try {
          localStorage.removeItem(`defai:${key}`);
        } catch (removeError) {
          // Ignore removal errors
        }
        return null;
      }
    }
    return null;
  }

  _cleanupOldStorage() {
    if (typeof localStorage === 'undefined') return;
    
    try {
      const defaiKeys = [];
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key && key.startsWith('defai:')) {
          defaiKeys.push(key);
        }
      }
      
      // Remove old entries (keep only last 10)
      defaiKeys.sort().slice(0, -10).forEach(key => {
        try {
          localStorage.removeItem(key);
        } catch (error) {
          // Ignore individual removal errors
        }
      });
    } catch (error) {
      console.error('Storage cleanup failed:', error);
    }
  }

  _cleanupEventListeners() {
    try {
      // Clean up window event listeners
      if (this._windowListeners && typeof window !== 'undefined') {
        this._windowListeners.forEach(({ event, handler }) => {
          try {
            window.removeEventListener(event, handler);
          } catch (error) {
            console.error('Failed to remove window listener:', error);
          }
        });
        this._windowListeners = [];
      }

      // Clean up internal event handlers
      if (this._eventHandlers) {
        this._eventHandlers.clear();
      }

      // Clean up message handlers
      if (this._messageHandlers) {
        this._messageHandlers.clear();
      }
    } catch (error) {
      console.error('Event listener cleanup failed:', error);
    }
  }

  emitToOthers(event, data) {
    if (this.currentContext?.api?.emitToOthers) {
      this.currentContext.api.emitToOthers(event, data);
      return;
    }
    if (typeof window !== 'undefined') {
      window.dispatchEvent(new CustomEvent(`defai:${event}`, { detail: data }));
    }
  }

  onFromOthers(event, handler) {
    this._messageHandlers.set(event, handler);
    if (typeof window !== 'undefined') {
      const wrappedHandler = (e) => handler(e.detail);
      
      // Track listeners for cleanup
      if (!this._windowListeners) {
        this._windowListeners = [];
      }
      const listenerEntry = { event: `defai:${event}`, handler: wrappedHandler };
      this._windowListeners.push(listenerEntry);
      
      window.addEventListener(`defai:${event}`, wrappedHandler);
      
      // Return cleanup function that also removes from tracking
      return () => {
        window.removeEventListener(`defai:${event}`, wrappedHandler);
        const index = this._windowListeners.indexOf(listenerEntry);
        if (index > -1) {
          this._windowListeners.splice(index, 1);
        }
      };
    }
    return () => {};
  }

  // Event emitter methods for internal events
  on(event, handler) {
    if (!this._eventHandlers) {
      this._eventHandlers = new Map();
    }
    if (!this._eventHandlers.has(event)) {
      this._eventHandlers.set(event, []);
    }
    this._eventHandlers.get(event).push(handler);
    return this;
  }

  off(event, handler) {
    if (!this._eventHandlers || !this._eventHandlers.has(event)) return this;
    const handlers = this._eventHandlers.get(event);
    const index = handlers.indexOf(handler);
    if (index > -1) {
      handlers.splice(index, 1);
    }
    return this;
  }

  emit(event, data) {
    if (!this._eventHandlers || !this._eventHandlers.has(event)) return this;
    const handlers = this._eventHandlers.get(event);
    handlers.forEach(handler => {
      try {
        handler(data);
      } catch (error) {
        console.error(`Error in event handler for ${event}:`, error);
      }
    });
    return this;
  }

  showError(message, options = {}) {
    if (this.currentContext?.api?.showError) {
      this.currentContext.api.showError(message, options);
    } else {
      console.error(`[${this.metadata?.id || 'DefaiElement'}]`, message);
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('defai:error', {
          detail: { message, element: this.metadata?.id, options }
        }));
      }
    }
  }

  showSuccess(message, options = {}) {
    if (this.currentContext?.api?.showSuccess) {
      this.currentContext.api.showSuccess(message, options);
    } else {
      // Library code must not spam the console in production. Surface success
      // via a DOM event; only log when explicitly opted into debug mode.
      if (typeof window !== 'undefined' && window.__DEFAI_SDK_DEBUG__) {
        console.log(`[${this.metadata?.id || 'DefaiElement'}]`, message);
      }
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('defai:success', {
          detail: { message, element: this.metadata?.id, options }
        }));
      }
    }
  }

  async onMount(context) {
    this.currentContext = context;
  }

  async onUnmount() {
    this.currentContext = null;
  }

  async onResize(size) {}

  async onSettingsChange(settings) {}

  // Lifecycle wrapper methods for testing and platform integration
  async _mount(context) {
    this.currentContext = context;
    await this.onMount(context);
    return this;
  }

  async _unmount() {
    await this.onUnmount();
    
    // Clean up event listeners to prevent memory leaks
    this._cleanupEventListeners();
    
    this.currentContext = null;
    return this;
  }

  async _resize(size) {
    await this.onResize(size);
    return this;
  }

  get isMounted() {
    return this.currentContext !== null;
  }
}

export class ElementSDK {
  constructor(options = {}) {
    this._options = options;
    this._emitter = new ElementEventEmitter();
    this._generator = new ContractGenerator(this._emitter, {
      apiUrl: options.apiUrl || null,
    });
    this._deployer = new ContractDeployer(this._emitter);
    this._verifier = new ContractVerifier(this._emitter);
    this._gasEstimator = new GasEstimator(this._emitter);
    this._initialized = false;
    this._initTimestamp = Date.now();

    this._emitter.emit(SDK_EVENTS.SDK_INITIALIZED, {
      version: SDK_VERSION,
      options: { apiUrl: options.apiUrl || null },
    });
    this._initialized = true;
  }

  get version() {
    return SDK_VERSION;
  }

  get events() {
    return this._emitter;
  }

  get metrics() {
    return {
      generationCount: this._generator.generationCount,
      deploymentCount: this._deployer.deploymentCount,
      verificationCount: this._verifier.verificationCount,
      estimationCount: this._gasEstimator.estimationCount,
      uptimeMs: Date.now() - this._initTimestamp,
      eventLogSize: this._emitter.getEventLog().length,
    };
  }

  on(event, handler) {
    this._emitter.on(event, handler);
    return this;
  }

  off(event, handler) {
    this._emitter.off(event, handler);
    return this;
  }

  async generateContract(config) {
    return this._generator.generate(config);
  }

  async deployContract(code, options) {
    return this._deployer.deploy(code, options);
  }

  async verifyContract(address, chainId, options) {
    return this._verifier.verify(address, chainId, options);
  }

  async estimateGas(config) {
    return this._gasEstimator.estimate(config);
  }

  async checkVerificationStatus(address, chainId) {
    return this._verifier.checkVerificationStatus(address, chainId);
  }

  getEventLog(filter) {
    return this._emitter.getEventLog(filter);
  }
}

export function createElementSDK(options) {
  return new ElementSDK(options);
}

export {
  SDK_EVENTS,
  ElementEventEmitter,
  ContractGenerator,
  ContractDeployer,
  ContractVerifier,
  GasEstimator,
  SUPPORTED_BLOCKCHAINS,
  EVM_CHAIN_CONFIG,
  SOLANA_CLUSTER_CONFIG,
  isSolanaChain,
  isEVMChain,
  SDK_VERSION,
};
