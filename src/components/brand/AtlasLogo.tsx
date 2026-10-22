/** The Atlas mark: an indigo tile with the letter "A". */
export function AtlasLogo({ size = 20 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      aria-hidden="true"
      focusable="false"
    >
      <rect width="20" height="20" rx="4" fill="#6366f1" />
      <path
        d="M10 4.5 14.25 15.5h-2.1l-.95-2.6H8.8l-.95 2.6h-2.1L10 4.5Zm0 3.55-1.55 3.3h3.1L10 8.05Z"
        fill="#fff"
      />
    </svg>
  );
}
