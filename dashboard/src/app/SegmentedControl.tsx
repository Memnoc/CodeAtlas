// A two-or-more-way switch rendered as one pill of adjacent buttons. Used
// for the header's Overview | Learn and Domain | Structural controls.
//
// A radio group rather than buttons: these choose between views rather than
// performing an action, and a reader arriving by keyboard should be able to
// arrow between them the way every other radio group works.

export type SegmentedOption<T extends string> = {
  value: T;
  label: string;
  /** Shown on hover and to assistive technology, where the label alone is terse. */
  hint: string;
};

export function SegmentedControl<T extends string>({
  name,
  value,
  options,
  onChange,
}: {
  /** Group name — also the control's accessible name. */
  name: string;
  value: T;
  options: readonly SegmentedOption<T>[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segmented" role="radiogroup" aria-label={name}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={option.value === value}
          className={`segment${option.value === value ? " segment-on" : ""}`}
          title={option.hint}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
