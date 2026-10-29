// Interrupteur accessible : un bouton `role="switch"` dont `aria-checked`
// porte l'état, activable à la souris, à Espace et à Entrée.

interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Nom accessible (« Activer « Factures » »). */
  label: string;
  disabled?: boolean;
}

export function Switch({ checked, onCheckedChange, label, disabled = false }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className="switch"
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
    >
      <span className="switch__thumb" aria-hidden="true" />
    </button>
  );
}
