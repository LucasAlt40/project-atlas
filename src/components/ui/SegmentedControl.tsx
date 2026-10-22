import { Icon, type IconName } from './Icon';
import styles from './SegmentedControl.module.css';

export interface Segment<Value extends string> {
  value: Value;
  label: string;
  icon?: IconName;
  disabled?: boolean;
  title?: string;
}

interface Props<Value extends string> {
  /** Accessible name of the group. */
  label: string;
  segments: readonly Segment<Value>[];
  value: Value;
  onChange: (value: Value) => void;
  disabled?: boolean;
}

/** Two to four mutually exclusive options in one compact control ("Lista | Grade"). */
export function SegmentedControl<Value extends string>({
  label,
  segments,
  value,
  onChange,
  disabled = false,
}: Props<Value>) {
  return (
    <div className={styles.group} role="group" aria-label={label}>
      {segments.map((segment) => (
        <button
          key={segment.value}
          type="button"
          className={styles.segment}
          aria-pressed={segment.value === value}
          disabled={disabled || segment.disabled}
          {...(segment.title ? { title: segment.title } : {})}
          onClick={() => {
            if (segment.value !== value) onChange(segment.value);
          }}
        >
          {segment.icon && <Icon name={segment.icon} size={15} />}
          {segment.label}
        </button>
      ))}
    </div>
  );
}
