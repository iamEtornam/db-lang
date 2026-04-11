import type { NavGroup } from '~/types/nav'

export const navigationMenus: NavGroup[] = [
  {
    items: [
      {
        title: 'Query',
        href: '/',
        icon: 'lucide:terminal-square',
      },
      {
        title: 'Schema',
        href: '/schema',
        icon: 'lucide:table-2',
      },
      {
        title: 'History',
        href: '/history',
        icon: 'lucide:clock',
      },
    ],
  },
]

export const bottomMenuItems = [
  {
    title: 'Settings',
    href: '/settings',
    icon: 'lucide:settings',
  },
]
