import { useState } from 'react'
import { useLogin } from '@/utils/apis/auth'

export function LoginModal() {
  const [email] = useState('demo@example.com')
  const [password] = useState('secret')
  const { refetch: attemptLogin } = useLogin(email, password, false)

  const handleLogin = async () => {
    await attemptLogin()
  }

  return <button onClick={handleLogin}>Login</button>
}
