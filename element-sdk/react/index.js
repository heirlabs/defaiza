import React, { createContext, useContext, useRef, useState, useCallback, useEffect } from 'react';
import { createElementSDK, SDK_EVENTS } from '@defai/element-sdk';

const ElementSDKContext = createContext(null);

export function ElementProvider({ children, apiUrl, options = {} }) {
  const sdkRef = useRef(null);

  if (!sdkRef.current) {
    sdkRef.current = createElementSDK({ apiUrl, ...options });
  }

  return React.createElement(
    ElementSDKContext.Provider,
    { value: sdkRef.current },
    children
  );
}

export function useElementSDK() {
  const sdk = useContext(ElementSDKContext);
  if (!sdk) {
    throw new Error('useElementSDK must be used within an ElementProvider');
  }
  return sdk;
}

export function useContractGeneration() {
  const sdk = useElementSDK();
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  const generate = useCallback(async (config) => {
    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const output = await sdk.generateContract(config);
      setResult(output);
      return output;
    } catch (err) {
      setError(err);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [sdk]);

  const reset = useCallback(() => {
    setResult(null);
    setError(null);
    setLoading(false);
  }, []);

  return { generate, result, loading, error, reset };
}

export function useDeployment() {
  const sdk = useElementSDK();
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [txHash, setTxHash] = useState(null);

  useEffect(() => {
    const handleTxSent = (data) => setTxHash(data.txHash);
    sdk.on(SDK_EVENTS.DEPLOYMENT_TX_SENT, handleTxSent);
    return () => sdk.off(SDK_EVENTS.DEPLOYMENT_TX_SENT, handleTxSent);
  }, [sdk]);

  const deploy = useCallback(async (code, options) => {
    setLoading(true);
    setError(null);
    setResult(null);
    setTxHash(null);

    try {
      const output = await sdk.deployContract(code, options);
      setResult(output);
      return output;
    } catch (err) {
      setError(err);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [sdk]);

  const reset = useCallback(() => {
    setResult(null);
    setError(null);
    setTxHash(null);
    setLoading(false);
  }, []);

  return { deploy, result, loading, error, txHash, reset };
}

export function useGasEstimation() {
  const sdk = useElementSDK();
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  const estimate = useCallback(async (config) => {
    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const output = await sdk.estimateGas(config);
      setResult(output);
      return output;
    } catch (err) {
      setError(err);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [sdk]);

  const reset = useCallback(() => {
    setResult(null);
    setError(null);
    setLoading(false);
  }, []);

  return { estimate, result, loading, error, reset };
}

export function useContractVerification() {
  const sdk = useElementSDK();
  const [result, setResult] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  const verify = useCallback(async (address, chainId, options = {}) => {
    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const output = await sdk.verifyContract(address, chainId, options);
      setResult(output);
      return output;
    } catch (err) {
      setError(err);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [sdk]);

  const checkStatus = useCallback(async (address, chainId) => {
    setLoading(true);
    setError(null);

    try {
      const output = await sdk.checkVerificationStatus(address, chainId);
      setResult(output);
      return output;
    } catch (err) {
      setError(err);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [sdk]);

  const reset = useCallback(() => {
    setResult(null);
    setError(null);
    setLoading(false);
  }, []);

  return { verify, checkStatus, result, loading, error, reset };
}

export function useSDKEvents(eventName, handler) {
  const sdk = useElementSDK();

  useEffect(() => {
    if (!eventName || !handler) return;
    sdk.on(eventName, handler);
    return () => sdk.off(eventName, handler);
  }, [sdk, eventName, handler]);
}

export function useSDKMetrics(pollIntervalMs = 5000) {
  const sdk = useElementSDK();
  const [metrics, setMetrics] = useState(sdk.metrics);

  useEffect(() => {
    const interval = setInterval(() => {
      setMetrics(sdk.metrics);
    }, pollIntervalMs);
    return () => clearInterval(interval);
  }, [sdk, pollIntervalMs]);

  return metrics;
}

export { SDK_EVENTS };
