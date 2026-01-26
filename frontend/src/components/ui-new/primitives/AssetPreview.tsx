import { useCallback, useEffect } from 'react';
import {
  XIcon,
  ArrowLeftIcon,
  ArrowRightIcon,
  DownloadSimpleIcon,
} from '@phosphor-icons/react';
import { cn } from '@/lib/utils';
import type { WorkspaceAsset } from '@/lib/api';

interface AssetPreviewProps {
  asset: WorkspaceAsset;
  assets?: WorkspaceAsset[];
  onClose: () => void;
  onNavigate?: (asset: WorkspaceAsset) => void;
  className?: string;
}

export function AssetPreview({
  asset,
  assets = [],
  onClose,
  onNavigate,
  className,
}: AssetPreviewProps) {
  const isVideo = asset.asset_type === 'video';
  const currentIndex = assets.findIndex((a) => a.id === asset.id);
  const hasPrev = currentIndex > 0;
  const hasNext = currentIndex < assets.length - 1;

  const handlePrev = useCallback(() => {
    if (hasPrev && onNavigate) {
      onNavigate(assets[currentIndex - 1]);
    }
  }, [hasPrev, currentIndex, assets, onNavigate]);

  const handleNext = useCallback(() => {
    if (hasNext && onNavigate) {
      onNavigate(assets[currentIndex + 1]);
    }
  }, [hasNext, currentIndex, assets, onNavigate]);

  const handleDownload = useCallback(() => {
    const link = document.createElement('a');
    link.href = asset.url;
    link.download = asset.filename;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }, [asset]);

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'Escape':
          onClose();
          break;
        case 'ArrowLeft':
          handlePrev();
          break;
        case 'ArrowRight':
          handleNext();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, handlePrev, handleNext]);

  return (
    <div
      className={cn(
        'fixed inset-0 z-50 flex items-center justify-center bg-black/80',
        className
      )}
      onClick={onClose}
    >
      {/* Close button */}
      <button
        type="button"
        onClick={onClose}
        className="absolute top-4 right-4 p-2 rounded-full bg-black/50 text-white hover:bg-black/70 transition-colors z-10"
      >
        <XIcon className="size-6" />
      </button>

      {/* Download button */}
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          handleDownload();
        }}
        className="absolute top-4 right-16 p-2 rounded-full bg-black/50 text-white hover:bg-black/70 transition-colors z-10"
        title="Download"
      >
        <DownloadSimpleIcon className="size-6" />
      </button>

      {/* Navigation buttons */}
      {hasPrev && onNavigate && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            handlePrev();
          }}
          className="absolute left-4 top-1/2 -translate-y-1/2 p-3 rounded-full bg-black/50 text-white hover:bg-black/70 transition-colors z-10"
        >
          <ArrowLeftIcon className="size-6" />
        </button>
      )}

      {hasNext && onNavigate && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            handleNext();
          }}
          className="absolute right-4 top-1/2 -translate-y-1/2 p-3 rounded-full bg-black/50 text-white hover:bg-black/70 transition-colors z-10"
        >
          <ArrowRightIcon className="size-6" />
        </button>
      )}

      {/* Main content */}
      <div
        className="max-w-[90vw] max-h-[90vh] flex flex-col items-center"
        onClick={(e) => e.stopPropagation()}
      >
        {isVideo ? (
          <video
            src={asset.url}
            controls
            autoPlay
            className="max-w-full max-h-[80vh] rounded-lg"
          >
            Your browser does not support video playback.
          </video>
        ) : (
          <img
            src={asset.url}
            alt={asset.description || 'Screenshot'}
            className="max-w-full max-h-[80vh] object-contain rounded-lg"
          />
        )}

        {/* Caption */}
        <div className="mt-4 text-center text-white">
          {asset.description && (
            <p className="text-lg font-medium">{asset.description}</p>
          )}
          <p className="text-sm text-white/70">
            {new Date(asset.captured_at).toLocaleString()}
            {assets.length > 1 && ` · ${currentIndex + 1} of ${assets.length}`}
          </p>
        </div>
      </div>
    </div>
  );
}
