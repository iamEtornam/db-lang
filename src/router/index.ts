import { createRouter, createWebHistory } from 'vue-router'
import Login from '../components/Login.vue'
import Signup from '../components/Signup.vue'
import Pricing from '../components/Pricing.vue'
import Home from '../Home.vue'

const routes = [
    {
        path: '/',
        name: 'Login',
        component: Login
    },
    {
        path: '/signup',
        name: 'Signup',
        component: Signup
    },
    {
        path: '/pricing',
        name: 'Pricing',
        component: Pricing
    },
    {
        path: '/dashboard',
        name: 'Dashboard',
        component: Home
    }
]

const router = createRouter({
    history: createWebHistory(),
    routes
})

export default router
