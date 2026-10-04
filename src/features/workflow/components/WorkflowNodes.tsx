import { Handle, Position, type NodeProps, type NodeTypes } from '@xyflow/react';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { shortId } from '@/features/workspace/model/inspection';
import type { FlowNode } from '../model/graph';
import { conditionText } from '../model/graph';
import { NODE_STATUS_VIEW } from '../model/status';
import styles from './Workflow.module.css';

/** Initial size, so a node is drawn (and the view fitted) before the library has measured it. */
export const NODE_SIZE = { width: 232, height: 92 };

function StatusBadge({ status }: { status: NonNullable<FlowNode['data']['status']> }) {
  const { t } = useI18n();
  const view = NODE_STATUS_VIEW[status];
  return (
    <span className={styles.badge} data-status={status}>
      <span aria-hidden="true">{view.symbol}</span> {t(view.label)}
    </span>
  );
}

function Frame({
  node,
  selected,
  icon,
  kind,
  children,
}: {
  node: NodeProps<FlowNode>;
  selected: boolean;
  icon: string;
  kind: TranslationKey;
  children: React.ReactNode;
}) {
  const { t } = useI18n();
  const { data } = node;
  return (
    <div
      className={styles.node}
      data-kind={node.type}
      data-status={data.status ?? 'none'}
      data-selected={selected}
      data-invalid={data.invalid}
      aria-label={`${data.node.label} (${t(kind)})`}
    >
      <Handle type="target" position={Position.Top} isConnectable={node.isConnectable} />
      <div className={styles.nodeTitle}>
        <span aria-hidden="true">{icon}</span>
        <span className={styles.nodeLabel}>{data.node.label}</span>
      </div>
      {children}
      <Handle type="source" position={Position.Bottom} isConnectable={node.isConnectable} />
    </div>
  );
}

function Progress({ data }: { data: FlowNode['data'] }) {
  const { t } = useI18n();
  return (
    <>
      {data.status && <StatusBadge status={data.status} />}
      {data.attempt && (
        <span className={styles.nodeMeta}>
          {t('workflow.node.execution', { id: shortId(data.attempt.executionId) })}
          {data.attempt.number > 1 &&
            ` · ${t('workflow.node.attempt', { n: data.attempt.number })}`}
        </span>
      )}
      {data.loop && data.loop.iteration > 0 && (
        <span className={styles.nodeMeta}>
          {t('workflow.node.loop', { n: data.loop.iteration, max: data.loop.max })}
        </span>
      )}
    </>
  );
}

function AgentNodeView(props: NodeProps<FlowNode>) {
  const { t } = useI18n();
  const { data } = props;
  return (
    <Frame node={props} selected={props.selected} icon="🧠" kind="workflow.node.agent">
      <span className={styles.nodeMeta}>
        {data.agent ? `${data.agent.name} · ${data.agent.personality}` : t('workflow.node.noAgent')}
      </span>
      <Progress data={data} />
    </Frame>
  );
}

function ConditionNodeView(props: NodeProps<FlowNode>) {
  const { data } = props;
  const node = data.node;
  return (
    <Frame node={props} selected={props.selected} icon="⑂" kind="workflow.node.condition">
      {node.type === 'condition' && (
        <span className={styles.nodeMeta}>{conditionText(node.condition)}</span>
      )}
      <Progress data={data} />
    </Frame>
  );
}

function EndNodeView(props: NodeProps<FlowNode>) {
  const { t } = useI18n();
  const { data } = props;
  const node = data.node;
  return (
    <Frame node={props} selected={props.selected} icon="⏹" kind="workflow.node.end">
      {node.type === 'end' && (
        <span className={styles.nodeMeta}>
          {t(`workflow.end.${node.outcome}` as TranslationKey)}
        </span>
      )}
      <Progress data={data} />
    </Frame>
  );
}

export const NODE_TYPES = {
  agent: AgentNodeView,
  condition: ConditionNodeView,
  end: EndNodeView,
} satisfies NodeTypes;
