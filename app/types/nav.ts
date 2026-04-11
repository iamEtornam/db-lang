export interface NavItem {
  title: string
  href?: string
  icon?: string
  badge?: string | number
  disabled?: boolean
  external?: boolean
  children?: NavItem[]
}

export interface NavGroup {
  title?: string
  items: NavItem[]
}
