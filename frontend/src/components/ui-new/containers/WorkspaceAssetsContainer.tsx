import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SpinnerIcon, ImagesIcon } from '@phosphor-icons/react';
import { useWorkspaceContext } from '@/contexts/WorkspaceContext';
import { useWorkspaceAssets } from '@/hooks/useWorkspaceAssets';
import { AssetThumbnail } from '../primitives/AssetThumbnail';
import { AssetPreview } from '../primitives/AssetPreview';
import type { WorkspaceAsset } from '@/lib/api';

export function WorkspaceAssetsContainer() {
  const { t } = useTranslation('common');
  const { workspace } = useWorkspaceContext();
  const workspaceId = workspace?.id;

  const { assets, isLoading } = useWorkspaceAssets(workspaceId);
  const [selectedAsset, setSelectedAsset] = useState<WorkspaceAsset | null>(
    null
  );

  if (!workspaceId) {
    return (
      <div className="p-base text-low text-sm flex-1">
        {t('assets.selectWorkspace', 'Select a workspace to view assets')}
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center flex-1 p-base">
        <SpinnerIcon className="animate-spin h-5 w-5 text-low" />
      </div>
    );
  }

  if (assets.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center p-base text-center flex-1">
        <ImagesIcon className="size-8 text-low mb-2" />
        <p className="text-sm text-low">
          {t('assets.empty', 'No assets captured yet')}
        </p>
        <p className="text-xs text-low mt-1">
          {t(
            'assets.emptyHint',
            'The AI will capture screenshots and videos here when documenting UI changes'
          )}
        </p>
      </div>
    );
  }

  return (
    <>
      <div className="p-base flex-1 min-h-0 overflow-y-auto">
        <div className="grid grid-cols-2 gap-2">
          {assets.map((asset) => (
            <AssetThumbnail
              key={asset.id}
              asset={asset}
              onClick={() => setSelectedAsset(asset)}
            />
          ))}
        </div>
      </div>

      {/* Lightbox/Preview */}
      {selectedAsset && (
        <AssetPreview
          asset={selectedAsset}
          assets={assets}
          onClose={() => setSelectedAsset(null)}
          onNavigate={setSelectedAsset}
        />
      )}
    </>
  );
}
