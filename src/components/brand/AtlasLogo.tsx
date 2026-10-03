import { useId } from 'react';

/** The Atlas mark: a geometric chevron over a coordinate dot, on a dark tile. */
export function AtlasLogo({ size = 28 }: { size?: number }) {
  const gradientId = useId();
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      aria-hidden="true"
      focusable="false"
    >
      <rect width="48" height="48" rx="10" fill="#15191e" />
      <rect x="0.5" y="0.5" width="47" height="47" rx="9.5" stroke="#242c37" />
      <path d="M24 10L36 34H28.5L24 23L19.5 34H12L24 10Z" fill={`url(#${gradientId})`} />
      <circle cx="24" cy="28" r="2.5" fill="#60a5fa" />
      <path
        d="M17 38H31"
        stroke="#3b82f6"
        strokeWidth="2"
        strokeLinecap="round"
        strokeOpacity="0.8"
      />
      <defs>
        <linearGradient
          id={gradientId}
          x1="12"
          y1="10"
          x2="36"
          y2="34"
          gradientUnits="userSpaceOnUse"
        >
          <stop stopColor="#818cf8" />
          <stop offset="0.5" stopColor="#6366f1" />
          <stop offset="1" stopColor="#3b82f6" />
        </linearGradient>
      </defs>
    </svg>
  );
}
