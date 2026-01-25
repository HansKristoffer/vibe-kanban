import { memo } from 'react';
import {
  type DragEndEvent,
  KanbanBoard,
  KanbanCards,
  KanbanHeader,
  KanbanProvider,
} from '@/components/ui/shadcn-io/kanban';
import { TaskCard } from './TaskCard';
import { InboxItemCard } from './InboxItemCard';
import type { TaskStatus, TaskWithAttemptStatus, InboxItem } from 'shared/types';
import {
  statusBoardColors,
  statusLabels,
  inboxBoardColor,
  inboxLabel,
} from '@/utils/statusLabels';

export type KanbanColumns = Record<TaskStatus, TaskWithAttemptStatus[]>;

interface TaskKanbanBoardProps {
  columns: KanbanColumns;
  onDragEnd: (event: DragEndEvent) => void;
  onViewTaskDetails: (task: TaskWithAttemptStatus) => void;
  selectedTaskId?: string;
  onCreateTask?: () => void;
  projectId: string;
  // Inbox props
  inboxItems?: InboxItem[];
  onViewInboxItem?: (item: InboxItem) => void;
  onCreateInboxItem?: () => void;
  selectedInboxItemId?: string;
}

function TaskKanbanBoard({
  columns,
  onDragEnd,
  onViewTaskDetails,
  selectedTaskId,
  onCreateTask,
  projectId,
  inboxItems = [],
  onViewInboxItem,
  onCreateInboxItem,
  selectedInboxItemId,
}: TaskKanbanBoardProps) {
  return (
    <KanbanProvider onDragEnd={onDragEnd}>
      {/* Inbox column - appears first */}
      <KanbanBoard id="inbox">
        <KanbanHeader
          name={inboxLabel}
          color={inboxBoardColor}
          onAddTask={onCreateInboxItem}
        />
        <KanbanCards>
          {inboxItems.map((item, index) => (
            <InboxItemCard
              key={item.id}
              item={item}
              index={index}
              onViewDetails={onViewInboxItem || (() => {})}
              isOpen={selectedInboxItemId === item.id}
            />
          ))}
        </KanbanCards>
      </KanbanBoard>

      {/* Task columns */}
      {Object.entries(columns).map(([status, tasks]) => {
        const statusKey = status as TaskStatus;
        return (
          <KanbanBoard key={status} id={statusKey}>
            <KanbanHeader
              name={statusLabels[statusKey]}
              color={statusBoardColors[statusKey]}
              onAddTask={onCreateTask}
            />
            <KanbanCards>
              {tasks.map((task, index) => (
                <TaskCard
                  key={task.id}
                  task={task}
                  index={index}
                  status={statusKey}
                  onViewDetails={onViewTaskDetails}
                  isOpen={selectedTaskId === task.id}
                  projectId={projectId}
                />
              ))}
            </KanbanCards>
          </KanbanBoard>
        );
      })}
    </KanbanProvider>
  );
}

export default memo(TaskKanbanBoard);
