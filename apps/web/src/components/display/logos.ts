// SPDX-License-Identifier: GPL-3.0-or-later
// Logo: dark on Dark and OLED black, light on Light (spec 3.2). Single source: assets/logo/.
import dark from '../../../../../assets/logo/cometcast-logo-dark.svg'
import light from '../../../../../assets/logo/cometcast-logo-light.svg'
import type { Theme } from '../../core/appearance'

export const logoFor = (theme: Theme): string => (theme === 'light' ? light : dark)
