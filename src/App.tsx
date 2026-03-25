import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MainLayout } from '@/components/layout/MainLayout'
import { Dashboard } from '@/pages/Dashboard'
import { Settings } from '@/pages/Settings'
import { useAppStore } from '@/stores/useAppStore'

const queryClient = new QueryClient()

function AppContent() {
  const activePage = useAppStore((s) => s.activePage)

  return (
    <MainLayout>
      {activePage === 'dashboard' ? <Dashboard /> : <Settings />}
    </MainLayout>
  )
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
    </QueryClientProvider>
  )
}
