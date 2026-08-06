export const TRACK_HEIGHT = 24;

type TrackRowProps = React.HTMLAttributes<HTMLDivElement> & {
  children?: React.ReactNode;
};

export default function TrackRow({
  children = null,
  className = '',
  style,
  ...rest
}: TrackRowProps) {
  return (
    <div
      className={`border-border shrink-0 border-b ${className}`}
      style={{ height: TRACK_HEIGHT, ...style }}
      {...rest}
    >
      {children}
    </div>
  );
}
