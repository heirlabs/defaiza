import EventEmitter from 'eventemitter3';

const SDK_EVENTS = {
  CONTRACT_GENERATION_START: 'contract:generation:start',
  CONTRACT_GENERATION_COMPLETE: 'contract:generation:complete',
  CONTRACT_GENERATION_ERROR: 'contract:generation:error',
  DEPLOYMENT_START: 'deployment:start',
  DEPLOYMENT_TX_SENT: 'deployment:tx:sent',
  DEPLOYMENT_CONFIRMED: 'deployment:confirmed',
  DEPLOYMENT_ERROR: 'deployment:error',
  VERIFICATION_START: 'verification:start',
  VERIFICATION_COMPLETE: 'verification:complete',
  VERIFICATION_ERROR: 'verification:error',
  GAS_ESTIMATION_START: 'gas:estimation:start',
  GAS_ESTIMATION_COMPLETE: 'gas:estimation:complete',
  GAS_ESTIMATION_ERROR: 'gas:estimation:error',
  SDK_INITIALIZED: 'sdk:initialized',
  SDK_ERROR: 'sdk:error',
};

export class ElementEventEmitter extends EventEmitter {
  constructor() {
    super();
    this._eventLog = [];
    this._maxLogSize = 500;
  }

  emit(event, ...args) {
    this._eventLog.push({
      event,
      timestamp: Date.now(),
      data: args[0] || null,
    });

    if (this._eventLog.length > this._maxLogSize) {
      this._eventLog = this._eventLog.slice(-this._maxLogSize);
    }

    return super.emit(event, ...args);
  }

  getEventLog(filter) {
    if (!filter) return [...this._eventLog];

    return this._eventLog.filter(entry => {
      if (filter.event && entry.event !== filter.event) return false;
      if (filter.since && entry.timestamp < filter.since) return false;
      if (filter.until && entry.timestamp > filter.until) return false;
      return true;
    });
  }

  clearEventLog() {
    this._eventLog = [];
  }
}

export { SDK_EVENTS };
