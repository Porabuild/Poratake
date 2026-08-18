const HANDLE_BARS = [
  'top-0 left-0 h-1 w-5',
  'top-0 left-0 h-5 w-1',
  'top-0 right-0 h-1 w-5',
  'top-0 right-0 h-5 w-1',
  'bottom-0 left-0 h-1 w-5',
  'bottom-0 left-0 h-5 w-1',
  'bottom-0 right-0 h-1 w-5',
  'bottom-0 right-0 h-5 w-1',
  'top-0 left-1/2 h-1 w-5 -translate-x-1/2',
  'bottom-0 left-1/2 h-1 w-5 -translate-x-1/2',
  'top-1/2 left-0 h-5 w-1 -translate-y-1/2',
  'top-1/2 right-0 h-5 w-1 -translate-y-1/2',
];

export default function SelectionHandles() {
  return (
    <>
      {HANDLE_BARS.map(bar => (
        <span
          key={bar}
          className={`absolute bg-primary ring-1 ring-black/20 ${bar}`}
        />
      ))}
    </>
  );
}
