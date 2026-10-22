import styles from './SectionNav.module.css';

export interface SectionNavItem<Id extends string> {
  id: Id;
  label: string;
}

interface Props<Id extends string> {
  label: string;
  items: readonly SectionNavItem<Id>[];
  active: Id;
  onSelect: (id: Id) => void;
}

/** The index of a page's sections, on its left: one is shown at a time. */
export function SectionNav<Id extends string>({ label, items, active, onSelect }: Props<Id>) {
  return (
    <nav className={styles.nav} aria-label={label}>
      <ul className={styles.list}>
        {items.map((item) => (
          <li key={item.id}>
            <button
              type="button"
              className={styles.item}
              aria-current={item.id === active ? 'page' : undefined}
              onClick={() => {
                onSelect(item.id);
              }}
            >
              {item.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
