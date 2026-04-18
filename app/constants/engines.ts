export interface DatabaseEngine {
  id: string
  name: string
  icon: string
  defaultPort: number | null
  category: 'sql' | 'nosql'
  defaultDatabase: string
  placeholder: {
    host: string
    database: string
    username: string
  }
  description: string
}

export const engines: DatabaseEngine[] = [
  {
    id: 'postgres',
    name: 'PostgreSQL',
    icon: 'simple-icons:postgresql',
    defaultPort: 5432,
    category: 'sql',
    defaultDatabase: 'postgres',
    placeholder: { host: 'localhost', database: 'mydb', username: 'postgres' },
    description: 'Advanced open-source relational database',
  },
  {
    id: 'mysql',
    name: 'MySQL',
    icon: 'simple-icons:mysql',
    defaultPort: 3306,
    category: 'sql',
    defaultDatabase: 'mysql',
    placeholder: { host: 'localhost', database: 'mydb', username: 'root' },
    description: 'Popular open-source relational database',
  },
  {
    id: 'mariadb',
    name: 'MariaDB',
    icon: 'simple-icons:mariadb',
    defaultPort: 3306,
    category: 'sql',
    defaultDatabase: 'mysql',
    placeholder: { host: 'localhost', database: 'mydb', username: 'root' },
    description: 'MySQL-compatible open-source database',
  },
  {
    id: 'sqlite',
    name: 'SQLite',
    icon: 'simple-icons:sqlite',
    defaultPort: null,
    category: 'sql',
    defaultDatabase: '',
    placeholder: { host: '/path/to/database.db', database: '', username: '' },
    description: 'Lightweight file-based database',
  },
  {
    id: 'mongodb',
    name: 'MongoDB',
    icon: 'simple-icons:mongodb',
    defaultPort: 27017,
    category: 'nosql',
    defaultDatabase: 'test',
    placeholder: { host: 'localhost', database: 'mydb', username: '' },
    description: 'Document-oriented NoSQL database',
  },
  {
    id: 'redis',
    name: 'Redis',
    icon: 'simple-icons:redis',
    defaultPort: 6379,
    category: 'nosql',
    defaultDatabase: '0',
    placeholder: { host: 'localhost', database: '0', username: '' },
    description: 'In-memory key-value data store',
  },
  {
    id: 'firestore',
    name: 'Firestore',
    icon: 'simple-icons:firebase',
    defaultPort: null,
    category: 'nosql',
    defaultDatabase: '(default)',
    placeholder: { host: '', database: '(default)', username: '' },
    description: 'Firebase Cloud Firestore document database',
  },
  {
    id: 'firebase_rtdb',
    name: 'Realtime DB',
    icon: 'simple-icons:firebase',
    defaultPort: null,
    category: 'nosql',
    defaultDatabase: '',
    placeholder: { host: 'https://my-app-default-rtdb.firebaseio.com', database: '', username: '' },
    description: 'Firebase Realtime Database',
  },
] as const

export function getEngine(id: string): DatabaseEngine | undefined {
  return engines.find(e => e.id === id)
}

/// Returns true if the given string looks like a full MongoDB connection URI
/// (either the standard `mongodb://` form or the SRV `mongodb+srv://` form).
export function isMongoUri(value: string): boolean {
  const v = value.trim().toLowerCase()
  return v.startsWith('mongodb://') || v.startsWith('mongodb+srv://')
}

/// Atlas copies connection strings with `<db_password>` (and historically
/// `<password>`) as a placeholder the user is expected to replace. If the
/// placeholder is present and a password was provided separately, substitute
/// the URL-encoded password in. Otherwise, return the URI untouched.
export function substituteMongoPasswordPlaceholder(uri: string, password: string): string {
  if (!password) return uri
  const encoded = encodeURIComponent(password)
  return uri
    .replace(/<db_password>/gi, encoded)
    .replace(/<password>/gi, encoded)
}

export function buildConnectionString(conn: {
  db_type: string
  host: string
  port: string
  database: string
  username: string
  password: string
}): string {
  const { db_type, host, port, database, username, password } = conn
  const encodedPwd = encodeURIComponent(password)
  const encodedUser = encodeURIComponent(username)

  switch (db_type) {
    case 'postgres':
      return `postgresql://${encodedUser}:${encodedPwd}@${host}:${port}/${database || 'postgres'}`
    case 'mysql':
    case 'mariadb':
      return `mysql://${encodedUser}:${encodedPwd}@${host}:${port}/${database || 'mysql'}`
    case 'sqlite':
      return host
    case 'mssql':
      return `mssql://${encodedUser}:${encodedPwd}@${host}:${port}/${database || 'master'}`
    case 'mongodb':
      if (isMongoUri(host)) {
        return substituteMongoPasswordPlaceholder(host, password)
      }
      if (username && password) {
        return `mongodb://${encodedUser}:${encodedPwd}@${host}:${port}/${database || 'test'}`
      }
      return `mongodb://${host}:${port}/${database || 'test'}`
    case 'redis':
      if (password) {
        return `redis://:${encodedPwd}@${host}:${port}/${database || '0'}`
      }
      return `redis://${host}:${port}/${database || '0'}`
    case 'firestore':
      return `firebase://<${username || 'project-id'}>/${database || '(default)'}`
    case 'firebase_rtdb':
      return host || 'firebase://<rtdb-url>'
    default:
      return ''
  }
}
