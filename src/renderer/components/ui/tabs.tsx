import * as React from 'react';
import { Tabs as HeroTabs } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

interface TabsProps extends Omit<
  React.ComponentProps<typeof HeroTabs>,
  'selectedKey' | 'defaultSelectedKey' | 'onSelectionChange'
> {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
}

function Tabs({ value, defaultValue, onValueChange, ...props }: TabsProps) {
  return (
    <HeroTabs
      data-slot="tabs"
      variant="secondary"
      selectedKey={value}
      defaultSelectedKey={defaultValue}
      onSelectionChange={key => onValueChange?.(String(key))}
      {...props}
    />
  );
}

function TabsList({
  className,
  ...props
}: React.ComponentProps<typeof HeroTabs.List>) {
  return (
    <HeroTabs.List
      data-slot="tabs-list"
      className={cn('w-fit', className)}
      {...props}
    />
  );
}

function TabsListContainer({
  className,
  ...props
}: React.ComponentProps<typeof HeroTabs.ListContainer>) {
  return (
    <HeroTabs.ListContainer
      data-slot="tabs-list-container"
      className={className}
      {...props}
    />
  );
}

interface TabsTriggerProps extends Omit<
  React.ComponentProps<typeof HeroTabs.Tab>,
  'id' | 'isDisabled'
> {
  value: string;
  disabled?: boolean;
}

function TabsTrigger({
  className,
  value,
  disabled,
  ...props
}: TabsTriggerProps) {
  return (
    <HeroTabs.Tab
      data-slot="tabs-trigger"
      id={value}
      isDisabled={disabled}
      className={cn('text-sm font-medium', className)}
      {...props}
    />
  );
}

function TabsIndicator({
  className,
  ...props
}: React.ComponentProps<typeof HeroTabs.Indicator>) {
  return (
    <HeroTabs.Indicator
      data-slot="tabs-indicator"
      className={className}
      {...props}
    />
  );
}

interface TabsContentProps extends Omit<
  React.ComponentProps<typeof HeroTabs.Panel>,
  'id'
> {
  value: string;
}

function TabsContent({ className, value, ...props }: TabsContentProps) {
  return (
    <HeroTabs.Panel
      data-slot="tabs-content"
      id={value}
      className={cn('mt-2', className)}
      {...props}
    />
  );
}

export {
  Tabs,
  TabsContent,
  TabsIndicator,
  TabsList,
  TabsListContainer,
  TabsTrigger,
};
