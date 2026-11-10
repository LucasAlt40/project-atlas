import { useState } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import type { Day, Series } from '../model/report';
import styles from './Usage.module.css';

const WIDTH = 760;
const HEIGHT = 200;
const LEFT = 52;
const BOTTOM = 22;
const TOP = 8;

/** A round step for the value axis, so its labels read as numbers people use. */
function niceStep(max: number): number {
  const raw = max / 3;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 2.5, 5, 10]) if (raw <= m * magnitude) return m * magnitude;
  return 10 * magnitude;
}

/** Class names joined, the missing ones left out. */
export function cx(...names: (string | undefined | false)[]): string {
  return names.filter(Boolean).join(' ');
}

export function seriesClass(slot: number | null): string {
  return slot === null ? (styles.seriesOther ?? '') : (styles[`series${String(slot + 1)}`] ?? '');
}

/**
 * Tokens (or cost) per day as stacked bars, one color per series, the same series keeping the
 * same color in every view. Hovering a day shows its numbers; the legend names every color.
 */
export function UsageChart({
  days,
  series,
  format,
  label,
  othersLabel,
}: {
  days: Day[];
  series: Series[];
  format: (value: number) => string;
  label: string;
  othersLabel: string;
}) {
  const { language } = useI18n();
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(...days.map((d) => d.total), 0);
  const step = max > 0 ? niceStep(max) : 1;
  const top = max > 0 ? step * Math.ceil(max / step) : 1;
  const plotW = WIDTH - LEFT - 8;
  const plotH = HEIGHT - TOP - BOTTOM;
  const slotW = plotW / Math.max(days.length, 1);
  const barW = Math.max(2, Math.min(28, slotW - (days.length > 40 ? 1 : 4)));
  const ticks = Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);
  const labelEvery = Math.ceil(days.length / 8);
  const dayLabel = (ms: number) =>
    new Intl.DateTimeFormat(language, { day: '2-digit', month: '2-digit' }).format(ms);
  const nameOf = (s: Series) => (s.slot === null ? othersLabel : s.label);
  const hovered = hover === null ? null : days[hover];

  return (
    <div className={styles.chart}>
      <ul className={styles.legend}>
        {series.map((s) => (
          <li key={s.key}>
            <span className={cx(styles.swatch, seriesClass(s.slot))} aria-hidden="true" />
            {nameOf(s)}
          </li>
        ))}
      </ul>
      <div className={styles.plot}>
        <svg viewBox={`0 0 ${String(WIDTH)} ${String(HEIGHT)}`} role="img" aria-label={label}>
          {ticks.map((tick) => {
            const y = TOP + plotH - (plotH * tick) / top;
            return (
              <g key={tick}>
                <line className={styles.grid} x1={LEFT} x2={WIDTH - 8} y1={y} y2={y} />
                <text className={styles.axis} x={LEFT - 8} y={y + 4} textAnchor="end">
                  {format(tick)}
                </text>
              </g>
            );
          })}
          {days.map((day, i) => {
            const x = LEFT + i * slotW + (slotW - barW) / 2;
            let y = TOP + plotH;
            return (
              <g
                key={day.start}
                onMouseEnter={() => {
                  setHover(i);
                }}
                onMouseLeave={() => {
                  setHover(null);
                }}
              >
                <rect
                  className={styles.hit}
                  x={LEFT + i * slotW}
                  y={TOP}
                  width={slotW}
                  height={plotH}
                />
                {series.map((s) => {
                  const v = day.values.get(s.key) ?? 0;
                  if (v === 0) return null;
                  const h = Math.max(1, (plotH * v) / top);
                  y -= h;
                  // A 1px gap of background separates stacked segments.
                  return (
                    <rect
                      key={s.key}
                      className={seriesClass(s.slot)}
                      x={x}
                      y={y + 1}
                      width={barW}
                      height={Math.max(h - 1, 1)}
                      rx={1.5}
                      opacity={hover === null || hover === i ? 1 : 0.45}
                    />
                  );
                })}
                {(i === days.length - 1 ||
                  (i % labelEvery === 0 && days.length - 1 - i >= labelEvery / 2)) && (
                  <text className={styles.axis} x={x + barW / 2} y={HEIGHT - 6} textAnchor="middle">
                    {dayLabel(day.start)}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
        {hovered && hovered.runs > 0 && (
          <div className={styles.tooltip} role="status">
            <strong>{dayLabel(hovered.start)}</strong>
            <span>
              {format(hovered.total)} · {hovered.runs}×
            </span>
            {series
              .filter((s) => (hovered.values.get(s.key) ?? 0) > 0)
              .map((s) => (
                <span key={s.key} className={styles.tipRow}>
                  <span className={cx(styles.swatch, seriesClass(s.slot))} aria-hidden="true" />
                  {nameOf(s)}
                  <b>{format(hovered.values.get(s.key) ?? 0)}</b>
                </span>
              ))}
          </div>
        )}
      </div>
    </div>
  );
}
