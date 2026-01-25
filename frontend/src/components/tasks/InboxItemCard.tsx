import { useCallback, useEffect, useRef, useMemo } from 'react';
import { KanbanCard } from '@/components/ui/shadcn-io/kanban';
import { Clock, Loader2 } from 'lucide-react';
import type { InboxItem } from 'shared/types';
import { Badge } from '@/components/ui/badge';
import { TaskCardHeader } from './TaskCardHeader';

/** Check if PRD generation was requested but not yet completed */
function isPrdGenerating(item: InboxItem): boolean {
  if (item.prd_markdown) return false; // PRD already exists
  if (!item.raw_payload_json) return false;
  try {
    const payload = JSON.parse(item.raw_payload_json);
    return payload.generate_prd === true;
  } catch {
    return false;
  }
}

interface InboxItemCardProps {
  item: InboxItem;
  index: number;
  onViewDetails: (item: InboxItem) => void;
  isOpen?: boolean;
}

function formatTimeAgo(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  const diffMs = Date.now() - d.getTime();
  const absSec = Math.round(Math.abs(diffMs) / 1000);

  if (absSec < 60) return 'just now';
  const mins = Math.round(absSec / 60);
  if (mins === 1) return '1 min ago';
  if (mins < 60) return `${mins} mins ago`;
  const hours = Math.round(mins / 60);
  if (hours === 1) return '1 hour ago';
  if (hours < 24) return `${hours} hours ago`;
  const days = Math.round(hours / 24);
  if (days === 1) return '1 day ago';
  if (days < 30) return `${days} days ago`;
  const months = Math.round(days / 30);
  if (months === 1) return '1 month ago';
  if (months < 12) return `${months} months ago`;
  const years = Math.round(months / 12);
  if (years === 1) return '1 year ago';
  return `${years} years ago`;
}

function getKindBadgeVariant(kind: InboxItem['kind']) {
  switch (kind) {
    case 'bug':
      return 'destructive';
    case 'feature':
      return 'default';
    default:
      return 'secondary';
  }
}

export function InboxItemCard({
  item,
  index,
  onViewDetails,
  isOpen,
}: InboxItemCardProps) {
  const handleClick = useCallback(() => {
    onViewDetails(item);
  }, [item, onViewDetails]);

  const localRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen || !localRef.current) return;
    const el = localRef.current;
    requestAnimationFrame(() => {
      el.scrollIntoView({
        block: 'center',
        inline: 'nearest',
        behavior: 'smooth',
      });
    });
  }, [isOpen]);

  const prdGenerating = useMemo(() => isPrdGenerating(item), [item]);
  const description = item.prd_markdown || '';

  return (
    <KanbanCard
      key={item.id}
      id={item.id}
      name={item.title}
      index={index}
      parent="inbox"
      onClick={handleClick}
      isOpen={isOpen}
      forwardedRef={localRef}
      dragDisabled={true}
    >
      <div className="flex flex-col gap-2">
        <TaskCardHeader
          title={item.title}
          right={
            <div className="flex items-center gap-1">
              <span className="flex items-center gap-1 text-xs text-muted-foreground">
                <Clock className="h-3 w-3" />
                {formatTimeAgo(item.created_at)}
              </span>
            </div>
          }
        />
        {prdGenerating ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-3 w-3 animate-spin" />
            <span>Generating PRD...</span>
          </div>
        ) : description ? (
          <p className="text-sm text-secondary-foreground break-words">
            {description.length > 130
              ? `${description.substring(0, 130)}...`
              : description}
          </p>
        ) : null}
        <div className="flex items-center gap-1">
          <Badge variant="outline" className="text-xs">
            {item.source}
          </Badge>
          <Badge variant={getKindBadgeVariant(item.kind)} className="text-xs">
            {item.kind}
          </Badge>
        </div>
      </div>
    </KanbanCard>
  );
}
