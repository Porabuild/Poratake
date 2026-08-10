import * as React from 'react';
import { Tooltip as HeroTooltip } from '@heroui/react';

const TooltipDelayContext = React.createContext(150);

function TooltipProvider({
  delayDuration = 150,
  children,
}: {
  delayDuration?: number;
  children: React.ReactNode;
}) {
  return (
    <TooltipDelayContext.Provider value={delayDuration}>
      {children}
    </TooltipDelayContext.Provider>
  );
}

function Tooltip(props: React.ComponentProps<typeof HeroTooltip>) {
  const delay = React.useContext(TooltipDelayContext);
  return <HeroTooltip delay={delay} {...props} />;
}

function mergeRefs<T>(
  ...refs: Array<React.Ref<T> | undefined>
): React.RefCallback<T> {
  return value => {
    refs.forEach(ref => {
      if (typeof ref === 'function') {
        ref(value);
        return;
      }
      if (ref) {
        (ref as React.MutableRefObject<T | null>).current = value;
      }
    });
  };
}

function mergeTriggerProps(
  childProps: Record<string, unknown>,
  triggerProps: Record<string, unknown>
): Record<string, unknown> {
  const merged = { ...childProps, ...triggerProps };
  Object.keys(triggerProps).forEach(key => {
    const childHandler = childProps[key];
    const triggerHandler = triggerProps[key];
    if (
      !key.startsWith('on') ||
      typeof childHandler !== 'function' ||
      typeof triggerHandler !== 'function'
    ) {
      return;
    }
    merged[key] = (...args: unknown[]) => {
      childHandler(...args);
      triggerHandler(...args);
    };
  });
  return merged;
}

function TooltipTrigger({
  asChild,
  children,
  ...props
}: React.ComponentPropsWithRef<'div'> & { asChild?: boolean }) {
  if (!asChild) {
    return (
      <HeroTooltip.Trigger<'div'> {...props}>{children}</HeroTooltip.Trigger>
    );
  }

  const child = React.Children.only(children) as React.ReactElement<
    {
      className?: string;
      ref?: React.Ref<HTMLElement>;
    } & Record<string, unknown>
  >;

  return (
    <HeroTooltip.Trigger<'div'>
      {...props}
      render={triggerProps => {
        const mergedProps = mergeTriggerProps(
          child.props,
          triggerProps as unknown as Record<string, unknown>
        );
        return React.cloneElement(child, {
          ...mergedProps,
          className: [triggerProps.className, child.props.className]
            .filter(Boolean)
            .join(' '),
          ref: mergeRefs(
            triggerProps.ref as React.Ref<HTMLElement>,
            child.props.ref
          ),
        });
      }}
    />
  );
}

function TooltipContent({
  side,
  sideOffset,
  ...props
}: Omit<
  React.ComponentProps<typeof HeroTooltip.Content>,
  'placement' | 'offset'
> & {
  side?: 'top' | 'right' | 'bottom' | 'left';
  sideOffset?: number;
}) {
  return (
    <HeroTooltip.Content placement={side} offset={sideOffset} {...props} />
  );
}

export { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider };
