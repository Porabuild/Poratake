interface InputErrorProps {
  message?: string | null;
}

export function InputError({ message }: InputErrorProps) {
  if (!message) return null;

  return (
    <p className="text-destructive text-sm" role="alert">
      {message}
    </p>
  );
}
