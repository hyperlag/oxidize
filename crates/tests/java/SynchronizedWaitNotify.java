// Stage 21: this.wait() / this.notify() / this.notifyAll() inside
// synchronized methods and synchronized(this) blocks.
class WaitTask implements Runnable {
    private SynchronizedWaitNotify target;
    private boolean useBlock;

    public WaitTask(SynchronizedWaitNotify target, boolean useBlock) {
        this.target = target;
        this.useBlock = useBlock;
    }

    public void run() {
        try {
            if (useBlock) {
                target.waitFlagBlock();
            } else {
                target.waitFlagMethod();
            }
        } catch (InterruptedException e) {
            throw new RuntimeException(e);
        }
    }
}

class SynchronizedWaitNotify {
    private static boolean methodFlag = false;
    private static boolean methodWaiting = false;
    private static boolean blockFlag = false;
    private static boolean blockWaiting = false;

    // --- synchronized methods ---
    public synchronized void setFlagMethod() {
        methodFlag = true;
        this.notifyAll();           // this.notifyAll() in synchronized method
    }

    public synchronized void waitFlagMethod() throws InterruptedException {
        methodWaiting = true;
        this.notifyAll();
        while (!methodFlag) this.wait();  // this.wait() in synchronized method
    }

    public synchronized void awaitMethodWaiterReady() throws InterruptedException {
        while (!methodWaiting) this.wait();
    }

    // --- synchronized(this) blocks ---
    public void setFlagBlock() {
        synchronized (this) {
            blockFlag = true;
            this.notifyAll();       // this.notifyAll() in synchronized(this)
        }
    }

    public void waitFlagBlock() throws InterruptedException {
        synchronized (this) {
            blockWaiting = true;
            this.notifyAll();
            while (!blockFlag) this.wait(); // this.wait() in synchronized(this)
        }
    }

    public void awaitBlockWaiterReady() throws InterruptedException {
        synchronized (this) {
            while (!blockWaiting) this.wait();
        }
    }

    public static void main(String[] args) throws InterruptedException {
        // Test 1: this.wait()/this.notifyAll() in synchronized methods
        SynchronizedWaitNotify s1 = new SynchronizedWaitNotify();
        methodFlag = false;
        methodWaiting = false;
        Thread t1 = new Thread(new WaitTask(s1.clone(), false));
        t1.start();
        s1.awaitMethodWaiterReady();
        s1.setFlagMethod();
        t1.join();
        System.out.println("method ok");

        // Test 2: this.wait()/this.notifyAll() in synchronized(this) blocks
        SynchronizedWaitNotify s2 = new SynchronizedWaitNotify();
        blockFlag = false;
        blockWaiting = false;
        Thread t2 = new Thread(new WaitTask(s2.clone(), true));
        t2.start();
        s2.awaitBlockWaiterReady();
        s2.setFlagBlock();
        t2.join();
        System.out.println("block ok");
    }
}
