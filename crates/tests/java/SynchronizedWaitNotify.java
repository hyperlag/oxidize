// Stage 21: this.wait() / this.notify() / this.notifyAll() inside
// synchronized methods and synchronized(this) blocks.
class SynchronizedWaitNotify {
    private boolean flag = false;

    // --- synchronized methods ---
    public synchronized void setFlagMethod() {
        flag = true;
        this.notifyAll();           // this.notifyAll() in synchronized method
    }

    public synchronized void waitFlagMethod() throws InterruptedException {
        while (!flag) this.wait();  // this.wait() in synchronized method
    }

    // --- synchronized(this) blocks ---
    public void setFlagBlock() {
        synchronized (this) {
            flag = true;
            this.notifyAll();       // this.notifyAll() in synchronized(this)
        }
    }

    public void waitFlagBlock() throws InterruptedException {
        synchronized (this) {
            while (!flag) this.wait(); // this.wait() in synchronized(this)
        }
    }

    public static void main(String[] args) throws InterruptedException {
        // Test 1: this.wait()/this.notifyAll() in synchronized methods
        SynchronizedWaitNotify s1 = new SynchronizedWaitNotify();
        s1.setFlagMethod();
        s1.waitFlagMethod();
        System.out.println("method ok");

        // Test 2: this.wait()/this.notifyAll() in synchronized(this) blocks
        SynchronizedWaitNotify s2 = new SynchronizedWaitNotify();
        s2.setFlagBlock();
        s2.waitFlagBlock();
        System.out.println("block ok");
    }
}
