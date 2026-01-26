import { useEffect, useRef, useState } from 'react';
import { stripAnsi } from 'fancy-ansi';

export interface PreviewUrlInfo {
  url: string;
  port?: number;
  scheme: 'http' | 'https';
}

// Explicit frontend port indicator - highest priority
// Projects can output "using_frontend_port:5174" to explicitly specify the dev server port
const explicitPortPattern = /using_frontend_port[:\s]+(\d{2,5})/i;

const urlPatterns = [
  // Full URL pattern (e.g., http://localhost:3000, https://127.0.0.1:8080)
  /(https?:\/\/(?:\[[0-9a-f:]+\]|localhost|127\.0\.0\.1|0\.0\.0\.0|\d{1,3}(?:\.\d{1,3}){3})(?::\d{2,5})?(?:\/\S*)?)/i,
  // Host:port pattern (e.g., localhost:3000, 0.0.0.0:8080)
  /(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[[0-9a-f:]+\]|(?:\d{1,3}\.){3}\d{1,3}):(\d{2,5})/i,
];

// Patterns that indicate the line is about a database/service, not a web server
// These should be skipped when detecting dev server URLs
const serviceIndicators =
  /\b(postgres|postgresql|mysql|mariadb|redis|clickhouse|mongodb|mongo|elasticsearch|rabbitmq|kafka|memcached|sqlite|database|db)\s*:/i;

// Get the hostname from the current browser location, falling back to 'localhost'
const getBrowserHostname = (): string => {
  if (typeof window !== 'undefined') {
    return window.location.hostname;
  }
  return 'localhost';
};

// Get the protocol from the current browser location
// When accessed via HTTPS (e.g., ngrok), we must use HTTPS for iframes to avoid Mixed Content errors
const getBrowserScheme = (): 'http' | 'https' => {
  if (typeof window !== 'undefined') {
    return window.location.protocol === 'https:' ? 'https' : 'http';
  }
  return 'http';
};

export const detectPreviewUrl = (line: string): PreviewUrlInfo | null => {
  const cleaned = stripAnsi(line);
  const browserHostname = getBrowserHostname();
  const browserScheme = getBrowserScheme();

  // Priority 1: Check for explicit frontend port indicator
  // e.g., "using_frontend_port:5174" or "using_frontend_port: 5174"
  const explicitMatch = explicitPortPattern.exec(cleaned);
  if (explicitMatch) {
    const port = Number(explicitMatch[1]);
    return {
      url: `${browserScheme}://${browserHostname}:${port}`,
      port,
      scheme: browserScheme,
    };
  }

  // Skip lines that mention database/service names - these are not web server URLs
  // e.g., "Postgres: localhost:5497" or "Redis: localhost:6379"
  if (serviceIndicators.test(cleaned)) {
    return null;
  }

  // Try to match a full URL first
  const fullUrlMatch = urlPatterns[0].exec(cleaned);
  if (fullUrlMatch) {
    try {
      const parsed = new URL(fullUrlMatch[1]);

      // Reject localhost/loopback URLs without a port - they're not valid dev server URLs
      const isLocalhost = [
        'localhost',
        '127.0.0.1',
        '0.0.0.0',
        '::',
        '[::]',
      ].includes(parsed.hostname);

      if (isLocalhost && !parsed.port) {
        // Fall through to host:port pattern detection
      } else {
        // Replace 0.0.0.0 or :: with browser hostname
        const needsHostnameReplacement =
          parsed.hostname === '0.0.0.0' ||
          parsed.hostname === '::' ||
          parsed.hostname === '[::]';

        if (needsHostnameReplacement) {
          parsed.hostname = browserHostname;
        }

        // When replacing hostname with browser hostname, inherit the browser's scheme
        // to avoid Mixed Content errors when accessed via HTTPS (e.g., ngrok)
        const browserScheme = getBrowserScheme();
        const scheme = needsHostnameReplacement
          ? browserScheme
          : parsed.protocol === 'https:'
            ? 'https'
            : 'http';

        if (needsHostnameReplacement) {
          parsed.protocol = `${scheme}:`;
        }

        return {
          url: parsed.toString(),
          port: parsed.port ? Number(parsed.port) : undefined,
          scheme,
        };
      }
    } catch {
      // Ignore invalid URLs and fall through to host:port detection
    }
  }

  // Try to match host:port pattern
  const hostPortMatch = urlPatterns[1].exec(cleaned);
  if (hostPortMatch) {
    const port = Number(hostPortMatch[1]);
    // Use browser's scheme to avoid Mixed Content errors when accessed via HTTPS
    const browserScheme = getBrowserScheme();
    const scheme =
      browserScheme === 'https'
        ? 'https'
        : /https/i.test(cleaned)
          ? 'https'
          : 'http';
    return {
      url: `${scheme}://${browserHostname}:${port}`,
      port,
      scheme,
    };
  }

  return null;
};

// Check if a log line contains an explicit frontend port indicator
const hasExplicitPort = (line: string): boolean => {
  return explicitPortPattern.test(stripAnsi(line));
};

export function usePreviewUrl(
  logs: Array<{ content: string }> | undefined
): PreviewUrlInfo | undefined {
  const [urlInfo, setUrlInfo] = useState<PreviewUrlInfo | undefined>();
  const [hasExplicitPortDetected, setHasExplicitPortDetected] = useState(false);
  const lastIndexRef = useRef(0);
  const lastExplicitCheckIndexRef = useRef(0);

  useEffect(() => {
    if (!logs) {
      setUrlInfo(undefined);
      setHasExplicitPortDetected(false);
      lastIndexRef.current = 0;
      lastExplicitCheckIndexRef.current = 0;
      return;
    }

    // Reset if logs were cleared (new process started)
    if (logs.length < lastIndexRef.current) {
      lastIndexRef.current = 0;
      lastExplicitCheckIndexRef.current = 0;
      setUrlInfo(undefined);
      setHasExplicitPortDetected(false);
    }

    // Always check new entries for explicit port indicator (can override previous URL)
    const newEntriesForExplicit = logs.slice(lastExplicitCheckIndexRef.current);
    for (const entry of newEntriesForExplicit) {
      if (hasExplicitPort(entry.content)) {
        const detected = detectPreviewUrl(entry.content);
        if (detected) {
          setUrlInfo(detected);
          setHasExplicitPortDetected(true);
          break;
        }
      }
    }
    lastExplicitCheckIndexRef.current = logs.length;

    // If we already have a URL from explicit port, skip regular detection
    if (hasExplicitPortDetected) {
      lastIndexRef.current = logs.length;
      return;
    }

    // If we already have a URL from regular detection, skip
    if (urlInfo) {
      lastIndexRef.current = logs.length;
      return;
    }

    // Scan new log entries for URL using regular detection
    let detectedUrl: PreviewUrlInfo | undefined;
    const newEntries = logs.slice(lastIndexRef.current);
    newEntries.some((entry) => {
      const detected = detectPreviewUrl(entry.content);
      if (detected) {
        detectedUrl = detected;
        return true;
      }
      return false;
    });

    if (detectedUrl) {
      setUrlInfo((prev) => prev ?? detectedUrl);
    }

    lastIndexRef.current = logs.length;
  }, [logs, urlInfo, hasExplicitPortDetected]);

  return urlInfo;
}
