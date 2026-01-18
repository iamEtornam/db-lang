<script setup lang="ts">
// Pricing logic placeholder
</script>

<template>
  <div class="bg-background-light dark:bg-background-dark text-slate-900 dark:text-white min-h-screen selection:bg-primary/30 font-display">
    <!-- Top Navigation Bar -->
    <header class="sticky top-0 z-50 border-b border-slate-200 dark:border-white/10 bg-white/80 dark:bg-background-dark/80 backdrop-blur-md">
      <div class="max-w-7xl mx-auto px-6 h-16 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="size-8 bg-primary rounded flex items-center justify-center text-white">
            <span class="material-symbols-outlined">database</span>
          </div>
          <h1 class="font-bold text-lg tracking-tight">DB Lang</h1>
        </div>
        <nav class="hidden md:flex items-center gap-8">
          <a class="text-sm font-medium text-slate-500 dark:text-slate-400 hover:text-primary dark:hover:text-white transition-colors" href="#">Queries</a>
          <a class="text-sm font-medium text-slate-500 dark:text-slate-400 hover:text-primary dark:hover:text-white transition-colors" href="#">History</a>
          <a class="text-sm font-medium text-slate-500 dark:text-slate-400 hover:text-primary dark:hover:text-white transition-colors" href="#">Connectors</a>
          <a class="text-sm font-medium text-slate-500 dark:text-slate-400 hover:text-primary dark:hover:text-white transition-colors" href="#">Settings</a>
        </nav>
        <div class="flex items-center gap-4">
          <button class="text-sm font-semibold px-4 py-2 rounded-lg bg-slate-100 dark:bg-white/5 hover:bg-slate-200 dark:hover:bg-white/10 transition-colors">
            Sign Out
          </button>
          <div class="size-8 rounded-full bg-slate-200 dark:bg-white/10 border border-slate-300 dark:border-white/10 overflow-hidden">
            <img class="w-full h-full object-cover" data-alt="User profile avatar placeholder" src="https://lh3.googleusercontent.com/aida-public/AB6AXuCOSh5iFbRlfRwway7CSNCeT3q5FjyAv2UDVSMhLSW1vEw4_dOEL40xByRlfPZOTm8fUdXbUNT0KLQm384zKDMwZW6hgE8kGO-dSVnU9dDYrmKFR5A1zatJER0AhoYWtZpV2q5gkzmEgKfdLjPmEGNd8KhGQulZXrFBKVcQHxwrhw_t94Eb3wbk3O2p8XAgOoQMqFpYBuNwLM9NsDSFp4IHIcOP2VeB2LzWHAYwKs5a5z7BKn4zeo6agRtDcAZPu6fPEQhnq3ygKm8"/>
          </div>
        </div>
      </div>
    </header>

    <main class="max-w-6xl mx-auto px-6 py-16">
      <!-- Page Heading -->
      <div class="text-center mb-12">
        <h2 class="text-4xl md:text-5xl font-black tracking-tight mb-4">Choose your plan</h2>
        <p class="text-slate-500 dark:text-slate-400 text-lg max-w-2xl mx-auto">
          Unlock the full power of natural language querying with Gemini-powered SQL generation.
        </p>
      </div>

      <!-- Billing Toggle -->
      <div class="flex justify-center mb-16">
        <div class="inline-flex p-1 bg-slate-100 dark:bg-white/5 rounded-xl border border-slate-200 dark:border-white/10">
          <button class="px-6 py-2 text-sm font-semibold rounded-lg bg-white dark:bg-primary shadow-sm text-slate-900 dark:text-white cursor-pointer">
            Monthly
          </button>
          <button class="px-6 py-2 text-sm font-semibold rounded-lg text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-white transition-colors cursor-pointer">
            Yearly <span class="ml-1 text-xs text-primary dark:text-primary/80 font-bold">Save 20%</span>
          </button>
        </div>
      </div>

      <!-- Pricing Grid -->
      <div class="flex flex-wrap justify-center gap-8">
        <!-- Starter Plan -->
        <div class="glass-card rounded-2xl p-8 flex flex-col h-full relative overflow-hidden group w-full md:w-[calc(33.333%-1.5rem)] min-w-[300px]">
          <div class="mb-8">
            <h3 class="text-xl font-bold mb-2">Starter</h3>
            <p class="text-slate-500 dark:text-slate-400 text-sm">Perfect for individuals learning SQL or side projects.</p>
          </div>
          <div class="mb-8">
            <div class="flex items-baseline gap-1">
              <span class="text-4xl font-black tracking-tight">$0</span>
              <span class="text-slate-500 text-sm">/mo</span>
            </div>
          </div>
          <button class="w-full py-3 px-4 rounded-xl font-bold text-sm bg-slate-100 dark:bg-white/5 text-slate-900 dark:text-white border border-slate-200 dark:border-white/10 hover:bg-slate-200 dark:hover:bg-white/10 transition-all mb-10 cursor-pointer">
            Current Plan
          </button>
          <div class="space-y-4 flex-grow">
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>50 Queries per month</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Standard Schema Support</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Community Access</span>
            </div>
            <div class="flex items-start gap-3 text-sm text-slate-400 dark:text-slate-500">
              <span class="material-symbols-outlined text-lg">block</span>
              <span>Custom Adapters</span>
            </div>
          </div>
        </div>

        <!-- Pro Plan (Featured) -->
        <div class="glass-card rounded-2xl p-8 flex flex-col h-full relative pro-glow border-primary/30 ring-1 ring-primary/20 w-full md:w-[calc(33.333%-1.5rem)] min-w-[300px]">
          <div class="absolute top-0 right-0">
            <div class="bg-primary text-white text-[10px] uppercase tracking-widest font-black px-4 py-1 rounded-bl-xl">
              Recommended
            </div>
          </div>
          <div class="mb-8">
            <h3 class="text-xl font-bold mb-2 text-primary">Pro</h3>
            <p class="text-slate-500 dark:text-slate-400 text-sm">The full experience for data analysts and developers.</p>
          </div>
          <div class="mb-8">
            <div class="flex items-baseline gap-1">
              <span class="text-4xl font-black tracking-tight">$29</span>
              <span class="text-slate-500 text-sm">/mo</span>
            </div>
          </div>
          <button class="w-full py-3 px-4 rounded-xl font-bold text-sm bg-primary text-white hover:brightness-110 shadow-lg shadow-primary/20 transition-all mb-10 cursor-pointer">
            Upgrade to Pro
          </button>
          <div class="space-y-4 flex-grow">
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span class="font-medium">Unlimited Gemini Queries</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Custom Database Adapters</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Full Query History & Search</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Advanced Data Visualization</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Multi-Database Syncing</span>
            </div>
          </div>
        </div>

        <!-- Enterprise Plan -->
        <div class="glass-card rounded-2xl p-8 flex flex-col h-full relative w-full md:w-[calc(33.333%-1.5rem)] min-w-[300px]">
          <div class="mb-8">
            <h3 class="text-xl font-bold mb-2">Enterprise</h3>
            <p class="text-slate-500 dark:text-slate-400 text-sm">Compliance and scale for large organizations.</p>
          </div>
          <div class="mb-8">
            <div class="flex items-baseline gap-1">
              <span class="text-4xl font-black tracking-tight">Custom</span>
            </div>
          </div>
          <button class="w-full py-3 px-4 rounded-xl font-bold text-sm bg-slate-900 dark:bg-white text-white dark:text-slate-900 hover:opacity-90 transition-all mb-10 cursor-pointer">
            Contact Sales
          </button>
          <div class="space-y-4 flex-grow">
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>SSO / SAML Support</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Dedicated Infrastructure</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>24/7 Priority Support</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Custom Security Training</span>
            </div>
            <div class="flex items-start gap-3 text-sm">
              <span class="material-symbols-outlined text-primary text-lg">check_circle</span>
              <span>Audit Logs & Monitoring</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Section Header: Comparison -->
      <div class="mt-24 mb-10 border-t border-slate-200 dark:border-white/5 pt-16">
        <h2 class="text-2xl font-bold mb-4">Frequently Asked Questions</h2>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-10">
          <div class="space-y-2">
            <h4 class="font-bold">Can I cancel my subscription?</h4>
            <p class="text-sm text-slate-500 dark:text-slate-400">Yes, you can cancel your subscription at any time. You will continue to have access to your plan until the end of your billing cycle.</p>
          </div>
          <div class="space-y-2">
            <h4 class="font-bold">Are my database credentials safe?</h4>
            <p class="text-sm text-slate-500 dark:text-slate-400">Absolutely. We use enterprise-grade encryption for all database connections. We never store your raw data, only metadata for SQL generation.</p>
          </div>
          <div class="space-y-2">
            <h4 class="font-bold">Do you support PostgreSQL and MySQL?</h4>
            <p class="text-sm text-slate-500 dark:text-slate-400">Yes, we support PostgreSQL, MySQL, SQLite, and MongoDB. Enterprise plans can request custom adapters for legacy systems.</p>
          </div>
          <div class="space-y-2">
            <h4 class="font-bold">How does the Gemini integration work?</h4>
            <p class="text-sm text-slate-500 dark:text-slate-400">We use Gemini 1.5 Pro to understand your natural language intent and map it to your specific database schema for high-precision SQL output.</p>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <footer class="mt-24 pt-8 border-t border-slate-200 dark:border-white/5 text-center">
        <div class="flex items-center justify-center gap-2 mb-4">
          <span class="material-symbols-outlined text-primary text-lg">security</span>
          <p class="text-xs font-medium text-slate-500 dark:text-slate-400 uppercase tracking-widest">Secure payment via Stripe · Cancel Anytime</p>
        </div>
        <p class="text-[10px] text-slate-400 dark:text-slate-600 max-w-lg mx-auto leading-relaxed">
          By subscribing, you agree to our Terms of Service and Privacy Policy. Gemini is a trademark of Google LLC. SQL generation accuracy may vary based on schema complexity.
        </p>
      </footer>
    </main>
  </div>
</template>

<style scoped>
.glass-card {
    background: rgba(29, 35, 38, 0.7);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.08);
}
.pro-glow {
    box-shadow: 0 0 40px -10px rgba(31, 138, 173, 0.3);
}
.material-symbols-outlined {
    font-variation-settings: 'FILL' 0, 'wght' 400, 'GRAD' 0, 'opsz' 20;
}
</style>
