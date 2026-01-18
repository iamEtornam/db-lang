<script setup lang="ts">
import { useRouter } from 'vue-router';

const router = useRouter();

const handleLogin = () => {
  // Placeholder for auth logic
  console.log("Logging in...");
  router.push('/dashboard');
};
</script>

<template>
  <div class="relative min-h-screen w-full flex items-center justify-center mac-os-bg p-6 overflow-y-auto bg-background-light dark:bg-background-dark font-display antialiased">
    <!-- Subtle Ambient Elements -->
    <div class="absolute top-[-10%] left-[-5%] w-[40%] h-[40%] bg-primary/20 rounded-full blur-[120px] pointer-events-none"></div>
    <div class="absolute bottom-[-10%] right-[-5%] w-[40%] h-[40%] bg-primary/10 rounded-full blur-[120px] pointer-events-none"></div>

    <!-- Main Auth Card -->
    <div class="layout-content-container flex flex-col w-full max-w-[440px] z-10">
      <div class="glass-card rounded-xl p-8 md:p-10 flex flex-col gap-8">
        <!-- Brand Header -->
        <div class="flex flex-col items-center gap-4">
          <div class="size-14 bg-primary flex items-center justify-center rounded-xl shadow-lg shadow-primary/20">
            <svg class="text-white size-8" fill="none" viewBox="0 0 48 48" xmlns="http://www.w3.org/2000/svg">
              <path d="M42.4379 44C42.4379 44 36.0744 33.9038 41.1692 24C46.8624 12.9336 42.2078 4 42.2078 4L7.01134 4C7.01134 4 11.6577 12.932 5.96912 23.9969C0.876273 33.9029 7.27094 44 7.27094 44L42.4379 44Z" fill="currentColor"></path>
            </svg>
          </div>
          <div class="text-center">
            <h1 class="text-white text-2xl font-bold tracking-tight">Sign In to DB Lang</h1>
            <p class="text-[#9fb4b7] text-sm mt-2 font-normal">Connect your database to natural language.</p>
          </div>
        </div>

        <!-- Auth Form -->
        <div class="flex flex-col gap-5">
          <div class="flex flex-col gap-2">
            <label class="text-[#e0e7e8] text-sm font-medium ml-1">Email Address</label>
            <input class="form-input w-full rounded-lg text-white border border-[#3d4f51] bg-black/20 focus:border-primary focus:ring-0 h-12 placeholder:text-[#6a7a7c] px-4 text-base font-normal input-focus transition-all duration-200" placeholder="name@company.com" type="email"/>
          </div>
          <button @click="handleLogin" class="flex w-full items-center justify-center rounded-lg h-12 px-4 bg-primary hover:bg-[#21a6b7] transition-colors text-white text-sm font-bold shadow-md shadow-primary/10 cursor-pointer">
            Continue with Email
          </button>
        </div>

        <!-- Separator -->
        <div class="relative flex items-center py-2">
          <div class="flex-grow border-t border-[#3d4f51]"></div>
          <span class="flex-shrink mx-4 text-[#6a7a7c] text-xs font-medium uppercase tracking-widest">or sign in with</span>
          <div class="flex-grow border-t border-[#3d4f51]"></div>
        </div>

        <!-- OAuth Section -->
        <div class="grid grid-cols-2 gap-4">
          <button class="flex items-center justify-center gap-3 rounded-lg h-11 px-4 border border-[#3d4f51] bg-white/5 hover:bg-white/10 transition-colors text-[#e0e7e8] text-sm font-medium cursor-pointer">
            <span class="material-symbols-outlined text-lg" style="font-variation-settings: 'FILL' 1">google</span>
            Google
          </button>
          <button class="flex items-center justify-center gap-3 rounded-lg h-11 px-4 border border-[#3d4f51] bg-white/5 hover:bg-white/10 transition-colors text-[#e0e7e8] text-sm font-medium cursor-pointer">
            <span class="material-symbols-outlined text-lg">terminal</span>
            GitHub
          </button>
        </div>

        <!-- Footer -->
        <div class="pt-2 text-center">
          <p class="text-[#9fb4b7] text-sm">
            Don't have an account? 
            <router-link to="/signup" class="text-primary hover:text-primary/80 font-semibold ml-1 transition-colors underline-offset-4 hover:underline">Sign up</router-link>
          </p>
        </div>
      </div>
      
      <!-- Additional Links / App Version -->
      <div class="mt-8 flex justify-between items-center px-4 text-[#6a7a7c] text-xs font-medium">
        <div class="flex gap-4">
          <a class="hover:text-white transition-colors" href="#">Privacy Policy</a>
          <a class="hover:text-white transition-colors" href="#">Terms of Service</a>
        </div>
        <span>v2.4.0-stable</span>
      </div>
    </div>

    <!-- Background Image Decorative -->
    <div class="fixed inset-0 z-0 opacity-40 pointer-events-none">
      <div class="w-full h-full bg-center bg-no-repeat bg-cover" data-alt="Blurred macOS Sequoia dynamic wallpaper abstract" style='background-image: url("https://lh3.googleusercontent.com/aida-public/AB6AXuAt2ytGkKf2wVO-yhnNrlWzCeWG_jGcCe2bTT2iDdHPCH19bJNtOvtjrUkzcBM7OwrLrfr9JJgx26YgXlJsN7kgZgrALgR0RsSuTwNsXvakoO-G0eRFVGlmn9hpZH88cYCwTbrE39Haj0phEaAAacqqYqIlAKFLIDlMlDM3QmcggeYpbbSz2jHlM2eDwzL6qJDHzyx1wE_59F16lXXKH98gsjpw2TPNz6FrS-WSP8tIKIIapIiIQsoTDKfYpfS9YFY8vOKNQ9bLuq0");'>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mac-os-bg {
    background: radial-gradient(circle at 0% 0%, #1b8e9d 0%, transparent 40%),
                radial-gradient(circle at 100% 100%, #2a3637 0%, transparent 40%),
                radial-gradient(circle at 50% 50%, #16181d 0%, #000000 100%);
    background-size: cover;
}
.glass-card {
    background: rgba(32, 37, 45, 0.85);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
}
.input-focus:focus {
    box-shadow: 0 0 0 2px rgba(27, 142, 157, 0.4);
}
</style>
