import defaultMdxComponents from 'fumadocs-ui/mdx';
import { Tab, Tabs } from 'fumadocs-ui/components/tabs';
import { TypeTable } from 'fumadocs-ui/components/type-table';
import { Code } from '@/components/code';
import {
  ScrollyCoding,
  ScrollySteps,
  ScrollyStep,
  ScrollyCode,
} from '@/components/scrolly-coding';
import type { MDXComponents } from 'mdx/types';

export function getMDXComponents(components?: MDXComponents): MDXComponents {
  return {
    ...defaultMdxComponents,
    Tab,
    Tabs,
    TypeTable,
    Code,
    ScrollyCoding,
    ScrollySteps,
    ScrollyStep,
    ScrollyCode,
    ...components,
  };
}
