import { ImageIcon, VideoIcon } from '@phosphor-icons/react';
import { cn } from '@/lib/utils';
import type { WorkspaceAsset } from '@/lib/api';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';

interface AssetThumbnailProps {
  asset: WorkspaceAsset;
  onClick?: () => void;
  selected?: boolean;
  selectable?: boolean;
  onSelect?: (selected: boolean) => void;
  className?: string;
}

export function AssetThumbnail({
  asset,
  onClick,
  selected,
  selectable,
  onSelect,
  className,
}: AssetThumbnailProps) {
  const isVideo = asset.asset_type === 'video';

  const handleClick = () => {
    if (selectable && onSelect) {
      onSelect(!selected);
    } else if (onClick) {
      onClick();
    }
  };

  const formatSize = (bytes: number | null) => {
    if (!bytes) return '';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDuration = (ms: number | null) => {
    if (!ms) return '';
    const seconds = Math.floor(ms / 1000);
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return mins > 0 ? `${mins}:${secs.toString().padStart(2, '0')}` : `${secs}s`;
  };

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={handleClick}
          className={cn(
            'relative group rounded-md overflow-hidden border transition-all',
            'w-full aspect-video bg-tertiary',
            'hover:border-accent focus:outline-none focus:ring-2 focus:ring-accent',
            selected && 'ring-2 ring-accent border-accent',
            selectable && 'cursor-pointer',
            className
          )}
        >
          {isVideo ? (
            // Video placeholder with icon
            <div className="absolute inset-0 flex items-center justify-center bg-secondary">
              <VideoIcon className="size-8 text-low" weight="fill" />
              {asset.duration_ms && (
                <span className="absolute bottom-1 right-1 px-1 py-0.5 text-xs bg-black/70 text-white rounded">
                  {formatDuration(asset.duration_ms)}
                </span>
              )}
            </div>
          ) : (
            // Screenshot image
            <img
              src={asset.url}
              alt={asset.description || 'Screenshot'}
              className="absolute inset-0 w-full h-full object-cover"
              loading="lazy"
            />
          )}

          {/* Type indicator overlay */}
          <div className="absolute top-1 left-1 p-0.5 rounded bg-black/50">
            {isVideo ? (
              <VideoIcon className="size-3 text-white" weight="fill" />
            ) : (
              <ImageIcon className="size-3 text-white" weight="fill" />
            )}
          </div>

          {/* Selection checkbox for selectable mode */}
          {selectable && (
            <div
              className={cn(
                'absolute top-1 right-1 w-4 h-4 rounded border-2 transition-colors',
                selected
                  ? 'bg-accent border-accent'
                  : 'bg-white/80 border-gray-400'
              )}
            >
              {selected && (
                <svg
                  className="w-full h-full text-white"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M13.854 3.646a.5.5 0 0 1 0 .708l-7 7a.5.5 0 0 1-.708 0l-3.5-3.5a.5.5 0 1 1 .708-.708L6.5 10.293l6.646-6.647a.5.5 0 0 1 .708 0z" />
                </svg>
              )}
            </div>
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-xs">
        <div className="space-y-1">
          {asset.description && (
            <p className="font-medium">{asset.description}</p>
          )}
          <p className="text-xs text-low">
            {isVideo ? 'Video' : 'Screenshot'}
            {asset.size_bytes && ` · ${formatSize(asset.size_bytes)}`}
            {asset.duration_ms && ` · ${formatDuration(asset.duration_ms)}`}
          </p>
          <p className="text-xs text-low">
            {new Date(asset.captured_at).toLocaleString()}
          </p>
        </div>
      </TooltipContent>
    </Tooltip>
  );
}
