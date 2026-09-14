import { BoldPlugin, ItalicPlugin, UnderlinePlugin } from '@platejs/basic-nodes/react'
import {
  FontColorPlugin,
  FontFamilyPlugin,
  FontSizePlugin,
  TextAlignPlugin,
} from '@platejs/basic-styles/react'
import { KEYS } from 'platejs'
import { ParagraphPlugin } from 'platejs/react'

import { SlideTextParagraph } from './SlideTextParagraph'

export const plateTextPlugins = [
  ParagraphPlugin.withComponent(SlideTextParagraph),
  BoldPlugin,
  ItalicPlugin,
  UnderlinePlugin,
  FontColorPlugin,
  FontFamilyPlugin,
  FontSizePlugin,
  TextAlignPlugin.configure({
    inject: {
      nodeProps: {
        defaultNodeValue: 'left',
        nodeKey: 'align',
        styleKey: 'textAlign',
        validNodeValues: ['left', 'center', 'right'],
      },
      targetPlugins: [KEYS.p],
    },
  }),
]
