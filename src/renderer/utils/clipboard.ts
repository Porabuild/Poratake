export const copyImageToClipboard = async (image: string) => {
  const blob = await fetch(`data:image/png;base64,${image}`).then(res =>
    res.blob()
  );
  await navigator.clipboard.write([
    new ClipboardItem({
      'image/png': blob,
    }),
  ]);
};
